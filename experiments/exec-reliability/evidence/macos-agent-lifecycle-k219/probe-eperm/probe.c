#include <spawn.h>
#include <stdlib.h>
#include <signal.h>
#include <errno.h>
#include <stdio.h>
#include <string.h>
#include <sys/wait.h>
#include <time.h>
#include <unistd.h>
extern char **environ;
static double now(void){struct timespec t;clock_gettime(CLOCK_MONOTONIC,&t);return t.tv_sec+t.tv_nsec/1e9;}
int main(int argc,char**argv){
  int runs=atoi(argv[1]); int eperm_runs=0, unknown_final=0;
  for(int r=0;r<runs;r++){
    posix_spawnattr_t a; posix_spawnattr_init(&a); posix_spawnattr_setflags(&a,POSIX_SPAWN_SETSID);
    char*av[]={"/bin/bash","-c","trap '' TERM; sleep 30 & wait",NULL}; pid_t pid;
    posix_spawn(&pid,"/bin/bash",NULL,&a,av,environ); usleep(100000);
    killpg(pid,SIGTERM); usleep(50000); killpg(pid,SIGKILL);
    int st; waitpid(pid,&st,0); double t0=now(); int saw=0; int last=0;
    while(now()-t0<1.4){ if(killpg(pid,0)!=0){ last=errno; if(errno==ESRCH)break; if(errno==EPERM&&!saw){saw=1;printf("run %d EPERM at %.4fs\n",r,now()-t0);} } else last=0; usleep(1000);}
    if(saw){eperm_runs++; printf("run %d final %s at %.4fs\n",r,last==ESRCH?"ESRCH":strerror(last),now()-t0);}
    if(last!=ESRCH)unknown_final++;
  }
  printf("runs %d eperm_seen %d not_esrch_at_end %d\n",runs,eperm_runs,unknown_final); return 0;}
