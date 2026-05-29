using System.Diagnostics;
using TestAnywareAgent.Models;
using TestAnywareAgent.Services;

namespace TestAnywareAgent;

public static class SystemEndpoints
{
    public static void MapSystemEndpoints(this WebApplication app, WindowEnumerator windowEnumerator)
    {
        app.MapGet("/health", () =>
        {
            var accessible = false;
            try
            {
                // Check API capability — getting the desktop root succeeds if
                // UI Automation is functional, regardless of open window count.
                windowEnumerator.EnumerateWindows();
                accessible = true;
            }
            catch { }

            return Results.Json(new HealthResponse { Accessible = accessible });
        });

        app.MapPost("/exec", async (ExecRequest req) =>
        {
            var timeout = req.Timeout ?? 30;
            var detach = req.Detach ?? false;

            var process = new Process
            {
                StartInfo = new ProcessStartInfo
                {
                    FileName = "cmd.exe",
                    Arguments = $"/c {req.Command}",
                    UseShellExecute = false,
                    RedirectStandardOutput = !detach,
                    RedirectStandardError = !detach,
                    CreateNoWindow = true,
                }
            };

            process.Start();

            if (detach)
            {
                return Results.Json(new ExecResult
                {
                    ExitCode = 0,
                    Stdout = "",
                    Stderr = "",
                });
            }

            var stdoutTask = process.StandardOutput.ReadToEndAsync();
            var stderrTask = process.StandardError.ReadToEndAsync();

            var exited = process.WaitForExit(timeout * 1000);
            if (!exited)
            {
                try { process.Kill(entireProcessTree: true); } catch { }
                return Results.Json(new ExecResult
                {
                    ExitCode = -1,
                    Stdout = await stdoutTask,
                    Stderr = "Process timed out",
                });
            }

            return Results.Json(new ExecResult
            {
                ExitCode = process.ExitCode,
                Stdout = (await stdoutTask).TrimEnd(),
                Stderr = (await stderrTask).TrimEnd(),
            });
        });

        // ADR-0001: /upload and /download stream raw application/octet-stream
        // bytes. The path rides in a percent-encoded query parameter (ASP.NET
        // percent-decodes Request.Query), and neither end buffers the file.
        app.MapPost("/upload", async (HttpRequest request) =>
        {
            var path = request.Query["path"].ToString();
            string? tempPath = null;
            try
            {
                var dir = Path.GetDirectoryName(path);
                if (!string.IsNullOrEmpty(dir))
                    Directory.CreateDirectory(dir);

                // Temp file in the destination's own directory keeps the
                // rename on one volume so File.Move is atomic on NTFS.
                tempPath = Path.Combine(
                    string.IsNullOrEmpty(dir) ? "." : dir,
                    $".{Path.GetFileName(path)}.{Path.GetRandomFileName()}.tmp");

                await using (var temp = File.Create(tempPath))
                    await request.Body.CopyToAsync(temp);

                File.Move(tempPath, path, overwrite: true);
                tempPath = null;
                return Results.Json(new ActionResponse
                {
                    Success = true,
                    Message = $"Uploaded to {path}",
                });
            }
            catch (Exception ex)
            {
                // A truncated/failed transfer must never leave the temp behind
                // or clobber the destination — it was renamed only on success.
                if (tempPath is not null)
                    try { File.Delete(tempPath); } catch { }
                return Results.Json(new ErrorResponse
                {
                    Error = "upload_failed",
                    Details = ex.Message,
                }, statusCode: 400);
            }
        });

        app.MapPost("/download", (HttpRequest request) =>
        {
            var path = request.Query["path"].ToString();
            try
            {
                // Open eagerly so a missing/unreadable file surfaces as a JSON
                // ErrorResponse before any body bytes are streamed.
                var stream = File.OpenRead(path);
                return Results.Stream(stream, "application/octet-stream");
            }
            catch (Exception ex)
            {
                return Results.Json(new ErrorResponse
                {
                    Error = "download_failed",
                    Details = ex.Message,
                }, statusCode: 400);
            }
        });

        app.MapPost("/shutdown", () =>
        {
            Task.Run(async () =>
            {
                await Task.Delay(100);
                Process.Start(new ProcessStartInfo
                {
                    FileName = "shutdown",
                    Arguments = "/s /t 0",
                    UseShellExecute = false,
                    CreateNoWindow = true,
                });
            });
            return Results.Json(new ActionResponse
            {
                Success = true,
                Message = "Shutting down",
            });
        });
    }
}
