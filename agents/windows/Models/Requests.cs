using System.Text.Json.Serialization;

namespace TestAnywareAgent.Models;

public sealed class ElementQuery
{
    [JsonPropertyName("role")] public string? Role { get; set; }
    [JsonPropertyName("label")] public string? Label { get; set; }
    [JsonPropertyName("window")] public string? Window { get; set; }
    [JsonPropertyName("id")] public string? Id { get; set; }
    [JsonPropertyName("index")] public int? Index { get; set; }
}

public sealed class SnapshotRequest
{
    [JsonPropertyName("mode")] public string? Mode { get; set; }
    [JsonPropertyName("window")] public string? Window { get; set; }
    [JsonPropertyName("role")] public string? Role { get; set; }
    [JsonPropertyName("label")] public string? Label { get; set; }
    [JsonPropertyName("depth")] public int? Depth { get; set; }
}

public sealed class SetValueRequest
{
    [JsonPropertyName("role")] public string? Role { get; set; }
    [JsonPropertyName("label")] public string? Label { get; set; }
    [JsonPropertyName("window")] public string? Window { get; set; }
    [JsonPropertyName("id")] public string? Id { get; set; }
    [JsonPropertyName("index")] public int? Index { get; set; }
    [JsonPropertyName("value")] public string Value { get; set; } = "";
}

public sealed class WindowTarget
{
    [JsonPropertyName("window")] public string Window { get; set; } = "";
}

public sealed class WindowResizeRequest
{
    [JsonPropertyName("window")] public string Window { get; set; } = "";
    [JsonPropertyName("width")] public int Width { get; set; }
    [JsonPropertyName("height")] public int Height { get; set; }
}

public sealed class WindowMoveRequest
{
    [JsonPropertyName("window")] public string Window { get; set; } = "";
    [JsonPropertyName("x")] public int X { get; set; }
    [JsonPropertyName("y")] public int Y { get; set; }
}

public sealed class WaitRequest
{
    [JsonPropertyName("window")] public string? Window { get; set; }
    [JsonPropertyName("timeout")] public int? Timeout { get; set; }
}

public sealed class ExecRequest
{
    [JsonPropertyName("command")] public string Command { get; set; } = "";
    [JsonPropertyName("timeout")] public int? Timeout { get; set; }
    [JsonPropertyName("detach")] public bool? Detach { get; set; }
}

public sealed class UploadRequest
{
    [JsonPropertyName("path")] public string Path { get; set; } = "";
    [JsonPropertyName("content")] public string Content { get; set; } = "";
}

public sealed class DownloadRequest
{
    [JsonPropertyName("path")] public string Path { get; set; } = "";
}
