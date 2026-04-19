using System.Text.Json.Serialization;

namespace TestAnywareAgent.Models;

public sealed class WindowInfo
{
    [JsonPropertyName("title")]
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)]
    public string? Title { get; set; }

    [JsonPropertyName("windowType")]
    public string WindowType { get; set; } = "standard";

    [JsonPropertyName("sizeWidth")]
    public double SizeWidth { get; set; }

    [JsonPropertyName("sizeHeight")]
    public double SizeHeight { get; set; }

    [JsonPropertyName("positionX")]
    public double PositionX { get; set; }

    [JsonPropertyName("positionY")]
    public double PositionY { get; set; }

    [JsonPropertyName("appName")]
    public string AppName { get; set; } = "";

    [JsonPropertyName("focused")]
    public bool Focused { get; set; }

    [JsonPropertyName("elements")]
    [JsonIgnore(Condition = JsonIgnoreCondition.WhenWritingNull)]
    public List<ElementInfo>? Elements { get; set; }
}
