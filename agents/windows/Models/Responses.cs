using System.Text.Json.Serialization;

namespace TestAnywareAgent.Models;

public sealed class SnapshotResponse
{
    [JsonPropertyName("windows")]
    public List<WindowInfo> Windows { get; set; } = [];
}

public sealed class ActionResponse
{
    [JsonPropertyName("success")]
    public bool Success { get; set; }

    [JsonPropertyName("message")]
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)]
    public string? Message { get; set; }
}

public sealed class ErrorResponse
{
    [JsonPropertyName("error")]
    public string Error { get; set; } = "";

    [JsonPropertyName("details")]
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)]
    public string? Details { get; set; }
}

public sealed class HealthResponse
{
    [JsonPropertyName("accessible")]
    public bool Accessible { get; set; }

    [JsonPropertyName("platform")]
    public string Platform { get; set; } = "windows";
}

public sealed class ExecResult
{
    [JsonPropertyName("exitCode")]
    public int ExitCode { get; set; }

    [JsonPropertyName("stdout")]
    public string Stdout { get; set; } = "";

    [JsonPropertyName("stderr")]
    public string Stderr { get; set; } = "";
}

public sealed class DownloadResponse
{
    [JsonPropertyName("content")]
    public string Content { get; set; } = "";
}

public sealed class InspectResponse
{
    [JsonPropertyName("element")]
    public ElementInfo Element { get; set; } = new();

    [JsonPropertyName("fontFamily")]
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)]
    public string? FontFamily { get; set; }

    [JsonPropertyName("fontSize")]
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)]
    public double? FontSize { get; set; }

    [JsonPropertyName("fontWeight")]
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)]
    public string? FontWeight { get; set; }

    [JsonPropertyName("textColor")]
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)]
    public string? TextColor { get; set; }

    [JsonPropertyName("boundsX")]
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)]
    public double? BoundsX { get; set; }

    [JsonPropertyName("boundsY")]
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)]
    public double? BoundsY { get; set; }

    [JsonPropertyName("boundsWidth")]
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)]
    public double? BoundsWidth { get; set; }

    [JsonPropertyName("boundsHeight")]
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)]
    public double? BoundsHeight { get; set; }
}
