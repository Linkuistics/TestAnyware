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

        app.MapPost("/upload", (UploadRequest req) =>
        {
            try
            {
                var data = Convert.FromBase64String(req.Content);
                var dir = Path.GetDirectoryName(req.Path);
                if (!string.IsNullOrEmpty(dir))
                    Directory.CreateDirectory(dir);
                File.WriteAllBytes(req.Path, data);
                return Results.Json(new ActionResponse
                {
                    Success = true,
                    Message = $"Uploaded to {req.Path}",
                });
            }
            catch (Exception ex)
            {
                return Results.Json(new ActionResponse
                {
                    Success = false,
                    Message = $"Upload failed: {ex.Message}",
                });
            }
        });

        app.MapPost("/download", (DownloadRequest req) =>
        {
            try
            {
                var data = File.ReadAllBytes(req.Path);
                return Results.Json(new DownloadResponse
                {
                    Content = Convert.ToBase64String(data),
                });
            }
            catch (Exception ex)
            {
                return Results.Json(new ErrorResponse
                {
                    Error = $"Download failed: {ex.Message}",
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
