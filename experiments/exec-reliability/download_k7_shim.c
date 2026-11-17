/*
 * Download seam for leaf download-consistency-k7.
 *
 * This library is loaded into the unchanged macOS agent with DYLD_INSERT_LIBRARIES
 * (man 1 dyld). It replaces stat, lstat, fstatat, open, openat, pread, close and bind
 * through the dyld __interpose section. Calls made by this library itself reach the
 * real functions, because dyld does not interpose the interposing image.
 *
 * Environment:
 *   K7_TARGET_DIR  absolute directory. Only paths below it are traced or paused.
 *   K7_TRACE       JSON Lines trace file, opened with O_APPEND.
 *   K7_GATE        gate directory. The controller writes <gate>/arm as
 *                  "EVENT N GENERATION", where EVENT is stat, lstat, open or pread.
 *                  Each new GENERATION restarts the per-event counts. The Nth
 *                  matching call consumes the arm, writes <gate>/paused and waits
 *                  for <gate>/resume, for 60 s at most. stat, lstat and open pause
 *                  after the call returns. pread pauses before the call.
 *   K7_LOOPBACK    when "1", a bind to the IPv4 or IPv6 wildcard address binds to
 *                  the loopback address instead.
 */
#include <arpa/inet.h>
#include <errno.h>
#include <fcntl.h>
#include <netinet/in.h>
#include <pthread.h>
#include <stdarg.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/param.h>
#include <sys/socket.h>
#include <sys/stat.h>
#include <sys/types.h>
#include <time.h>
#include <unistd.h>

#define INTERPOSE(replacement, original)                                                   \
    __attribute__((used)) static const struct {                                            \
        const void *replacement_function;                                                  \
        const void *original_function;                                                     \
    } interpose_##original __attribute__((section("__DATA,__interpose"))) = {              \
        (const void *)(unsigned long)&replacement, (const void *)(unsigned long)&original}

#define MAX_FD 4096
#define PAUSE_LIMIT_MS 60000

static pthread_mutex_t lock = PTHREAD_MUTEX_INITIALIZER;
static unsigned char traced_fd[MAX_FD];

static const char *env(const char *name) {
    const char *value = getenv(name);
    return value && value[0] ? value : NULL;
}

static int below_target(const char *path) {
    const char *dir = env("K7_TARGET_DIR");
    if (!dir || !path) return 0;
    size_t n = strlen(dir);
    return strncmp(path, dir, n) == 0 && (path[n] == '/' || path[n] == '\0');
}

static void trace(const char *format, ...) {
    const char *file = env("K7_TRACE");
    if (!file) return;
    char body[1536];
    va_list args;
    va_start(args, format);
    vsnprintf(body, sizeof body, format, args);
    va_end(args);
    struct timespec now;
    clock_gettime(CLOCK_REALTIME, &now);
    uint64_t thread = 0;
    pthread_threadid_np(NULL, &thread);
    char line[2048];
    int length = snprintf(line, sizeof line, "{\"ns\":%lld,\"tid\":%llu,%s}\n",
                          (long long)now.tv_sec * 1000000000LL + now.tv_nsec,
                          (unsigned long long)thread, body);
    int fd = open(file, O_WRONLY | O_APPEND | O_CREAT, 0644);
    if (fd < 0) return;
    if (length > 0) write(fd, line, (size_t)length < sizeof line ? (size_t)length : sizeof line - 1);
    close(fd);
}

/* Paths in the trace are below K7_TARGET_DIR and hold only safe characters. */
static void gate_path(char *out, size_t size, const char *leaf) {
    snprintf(out, size, "%s/%s", env("K7_GATE"), leaf);
}

/* Count this call and consume the arm when it matches. Returns the count of this
 * event type in the current generation, negated when this call must pause. */
static const char *EVENTS[] = {"stat", "lstat", "open", "pread"};
static long counts[4];
static char generation[64];

static long count_and_arm(const char *event) {
    if (!env("K7_GATE")) return 0;
    char arm[MAXPATHLEN], consumed[MAXPATHLEN], spec[128] = {0};
    gate_path(arm, sizeof arm, "arm");
    gate_path(consumed, sizeof consumed, "arm.consumed");
    int index = -1;
    for (int i = 0; i < 4; i++)
        if (strcmp(event, EVENTS[i]) == 0) index = i;
    if (index < 0) return 0;
    pthread_mutex_lock(&lock);
    long nth = ++counts[index];
    int pause = 0;
    int fd = open(arm, O_RDONLY);
    if (fd >= 0) {
        ssize_t n = read(fd, spec, sizeof spec - 1);
        close(fd);
        char want[16] = {0}, gen[64] = {0};
        long target = 0;
        if (n > 0 && sscanf(spec, "%15s %ld %63s", want, &target, gen) == 3) {
            if (strcmp(gen, generation) != 0) {
                strcpy(generation, gen);
                memset(counts, 0, sizeof counts);
                nth = counts[index] = 1;
            }
            if (strcmp(want, event) == 0 && target == nth) {
                pause = 1;
                rename(arm, consumed);
            }
        }
    }
    pthread_mutex_unlock(&lock);
    return pause ? -nth : nth;
}

static void pause_here(const char *event, const char *path, long nth) {
    char paused[MAXPATHLEN], resume[MAXPATHLEN];
    gate_path(paused, sizeof paused, "paused");
    gate_path(resume, sizeof resume, "resume");
    int fd = open(paused, O_WRONLY | O_CREAT | O_TRUNC, 0644);
    if (fd >= 0) {
        dprintf(fd, "%s %ld %s\n", event, nth, path);
        close(fd);
    }
    trace("\"event\":\"pause\",\"at\":\"%s\",\"nth\":%ld,\"path\":\"%s\"", event, nth, path);
    long waited = 0;
    struct stat ignored;
    while (stat(resume, &ignored) != 0 && waited < PAUSE_LIMIT_MS) {
        usleep(5000);
        waited += 5;
    }
    int timed_out = waited >= PAUSE_LIMIT_MS;
    unlink(resume);
    unlink(paused);
    trace("\"event\":\"resume\",\"at\":\"%s\",\"waited_ms\":%ld,\"timed_out\":%s", event, waited,
          timed_out ? "true" : "false");
}

static void trace_stat(const char *call, const char *path, int rc, int error,
                       const struct stat *info) {
    if (rc == 0)
        trace("\"event\":\"%s\",\"path\":\"%s\",\"rc\":0,\"ino\":%llu,\"size\":%lld", call, path,
              (unsigned long long)info->st_ino, (long long)info->st_size);
    else
        trace("\"event\":\"%s\",\"path\":\"%s\",\"rc\":%d,\"errno\":%d", call, path, rc, error);
}

static int k7_stat(const char *path, struct stat *info) {
    int rc = stat(path, info);
    int error = errno;
    if (below_target(path)) {
        long nth = count_and_arm("stat");
        trace_stat("stat", path, rc, error, info);
        if (nth < 0) pause_here("stat", path, -nth);
    }
    errno = error;
    return rc;
}

static int k7_lstat(const char *path, struct stat *info) {
    int rc = lstat(path, info);
    int error = errno;
    if (below_target(path)) {
        long nth = count_and_arm("lstat");
        trace_stat("lstat", path, rc, error, info);
        if (nth < 0) pause_here("lstat", path, -nth);
    }
    errno = error;
    return rc;
}

static int k7_fstatat(int dirfd, const char *path, struct stat *info, int flags) {
    int rc = fstatat(dirfd, path, info, flags);
    int error = errno;
    if (dirfd == AT_FDCWD && below_target(path)) trace_stat("fstatat", path, rc, error, info);
    errno = error;
    return rc;
}

static void opened(const char *call, const char *path, int fd, int flags, int error) {
    if (fd < 0) {
        trace("\"event\":\"%s\",\"path\":\"%s\",\"fd\":%d,\"errno\":%d", call, path, fd, error);
        return;
    }
    struct stat info;
    fstat(fd, &info);
    pthread_mutex_lock(&lock);
    if (fd < MAX_FD) traced_fd[fd] = 1;
    pthread_mutex_unlock(&lock);
    long nth = count_and_arm("open");
    trace("\"event\":\"%s\",\"path\":\"%s\",\"fd\":%d,\"flags\":%d,\"ino\":%llu,\"size\":%lld",
          call, path, fd, flags, (unsigned long long)info.st_ino, (long long)info.st_size);
    if (nth < 0) pause_here("open", path, -nth);
}

static int k7_open(const char *path, int flags, ...) {
    mode_t mode = 0;
    if (flags & O_CREAT) {
        va_list args;
        va_start(args, flags);
        mode = (mode_t)va_arg(args, int);
        va_end(args);
    }
    int fd = open(path, flags, mode);
    int error = errno;
    if (below_target(path)) opened("open", path, fd, flags, error);
    errno = error;
    return fd;
}

static int k7_openat(int dirfd, const char *path, int flags, ...) {
    mode_t mode = 0;
    if (flags & O_CREAT) {
        va_list args;
        va_start(args, flags);
        mode = (mode_t)va_arg(args, int);
        va_end(args);
    }
    int fd = openat(dirfd, path, flags, mode);
    int error = errno;
    if (dirfd == AT_FDCWD && below_target(path)) opened("openat", path, fd, flags, error);
    errno = error;
    return fd;
}

static int is_traced(int fd) {
    pthread_mutex_lock(&lock);
    int traced = fd >= 0 && fd < MAX_FD && traced_fd[fd];
    pthread_mutex_unlock(&lock);
    return traced;
}

static ssize_t k7_pread(int fd, void *buffer, size_t count, off_t offset) {
    if (!is_traced(fd)) return pread(fd, buffer, count, offset);
    long nth = count_and_arm("pread");
    char path[MAXPATHLEN] = "";
    fcntl(fd, F_GETPATH, path);
    if (nth < 0) {
        nth = -nth;
        pause_here("pread", path, nth);
    }
    struct stat info;
    fstat(fd, &info);
    ssize_t n = pread(fd, buffer, count, offset);
    int error = errno;
    trace("\"event\":\"pread\",\"fd\":%d,\"nth\":%ld,\"ino\":%llu,\"size_before\":%lld,"
          "\"offset\":%lld,\"count\":%zu,\"returned\":%zd,\"errno\":%d",
          fd, nth, (unsigned long long)info.st_ino, (long long)info.st_size, (long long)offset,
          count, n, n < 0 ? error : 0);
    errno = error;
    return n;
}

static int k7_close(int fd) {
    int traced = is_traced(fd);
    if (traced) {
        pthread_mutex_lock(&lock);
        traced_fd[fd] = 0;
        pthread_mutex_unlock(&lock);
    }
    int rc = close(fd);
    int error = errno;
    if (traced) trace("\"event\":\"close\",\"fd\":%d,\"rc\":%d", fd, rc);
    errno = error;
    return rc;
}

static int k7_bind(int socket, const struct sockaddr *address, socklen_t length) {
    const char *loopback = env("K7_LOOPBACK");
    if (!loopback || strcmp(loopback, "1") != 0 || !address) return bind(socket, address, length);
    if (address->sa_family == AF_INET && length >= sizeof(struct sockaddr_in)) {
        struct sockaddr_in copy;
        memcpy(&copy, address, sizeof copy);
        int rewritten = copy.sin_addr.s_addr == htonl(INADDR_ANY);
        if (rewritten) copy.sin_addr.s_addr = htonl(INADDR_LOOPBACK);
        int rc = bind(socket, (const struct sockaddr *)&copy, sizeof copy);
        int error = errno;
        trace("\"event\":\"bind\",\"family\":\"inet\",\"port\":%d,\"rewritten\":%s,\"rc\":%d",
              ntohs(copy.sin_port), rewritten ? "true" : "false", rc);
        errno = error;
        return rc;
    }
    if (address->sa_family == AF_INET6 && length >= sizeof(struct sockaddr_in6)) {
        struct sockaddr_in6 copy;
        memcpy(&copy, address, sizeof copy);
        int rewritten = IN6_IS_ADDR_UNSPECIFIED(&copy.sin6_addr);
        if (rewritten) copy.sin6_addr = in6addr_loopback;
        int rc = bind(socket, (const struct sockaddr *)&copy, sizeof copy);
        int error = errno;
        trace("\"event\":\"bind\",\"family\":\"inet6\",\"port\":%d,\"rewritten\":%s,\"rc\":%d",
              ntohs(copy.sin6_port), rewritten ? "true" : "false", rc);
        errno = error;
        return rc;
    }
    return bind(socket, address, length);
}

INTERPOSE(k7_stat, stat);
INTERPOSE(k7_lstat, lstat);
INTERPOSE(k7_fstatat, fstatat);
INTERPOSE(k7_open, open);
INTERPOSE(k7_openat, openat);
INTERPOSE(k7_pread, pread);
INTERPOSE(k7_close, close);
INTERPOSE(k7_bind, bind);
