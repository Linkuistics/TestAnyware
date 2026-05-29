using TestAnywareAgent;
using TestAnywareAgent.Services;

var port = args.Length > 0 && int.TryParse(args[0], out var p) ? p : 8648;

var builder = WebApplication.CreateBuilder();
builder.WebHost.UseUrls($"http://0.0.0.0:{port}");
builder.Logging.SetMinimumLevel(LogLevel.Warning);

// ADR-0001: /upload streams the raw request body straight to disk, so the
// Kestrel default ~28.6 MiB body cap (the old effective Windows file cap)
// must be lifted — uploads are now memory-bound only, not body-size-capped.
builder.WebHost.ConfigureKestrel(options => options.Limits.MaxRequestBodySize = null);

var app = builder.Build();

using var windowEnumerator = new WindowEnumerator();
app.MapSystemEndpoints(windowEnumerator);
app.MapAccessibilityEndpoints(windowEnumerator);

Console.WriteLine($"testanyware-agent listening on http://0.0.0.0:{port}");
app.Run();
