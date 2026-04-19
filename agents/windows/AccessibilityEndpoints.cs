using FlaUI.Core.AutomationElements;
using TestAnywareAgent.Models;
using TestAnywareAgent.Services;

namespace TestAnywareAgent;

public static class AccessibilityEndpoints
{
    public static void MapAccessibilityEndpoints(this WebApplication app, WindowEnumerator windowEnumerator)
    {
        app.MapPost("/windows", () =>
        {
            var windows = windowEnumerator.EnumerateWindows();
            return Results.Json(new SnapshotResponse { Windows = windows });
        });

        app.MapPost("/snapshot", (SnapshotRequest req) =>
        {
            var mode = req.Mode ?? "interact";
            var depth = req.Depth ?? 3;
            UnifiedRole? roleFilter = req.Role != null ? RoleMapper.MapFromString(req.Role) : null;

            var windows = windowEnumerator.EnumerateWindows();
            if (req.Window != null)
                windows = windows.Where(w => WindowEnumerator.WindowInfoMatches(w, req.Window)).ToList();

            foreach (var win in windows)
            {
                var winElement = windowEnumerator.FindWindowByInfo(win);
                if (winElement == null) continue;

                var elements = UiaTreeWalker.Walk(winElement, depth, roleFilter, req.Label);
                win.Elements = mode switch
                {
                    "interact" => FilterInteractive(elements),
                    "layout" => FilterLayout(elements),
                    _ => elements,
                };
            }

            return Results.Json(new SnapshotResponse { Windows = windows });
        });

        app.MapPost("/inspect", (ElementQuery req) =>
        {
            UnifiedRole? roleFilter = req.Role != null ? RoleMapper.MapFromString(req.Role) : null;

            var windows = windowEnumerator.EnumerateWindows();
            if (req.Window != null)
                windows = windows.Where(w => WindowEnumerator.WindowInfoMatches(w, req.Window)).ToList();

            var allElements = new List<ElementInfo>();
            foreach (var win in windows)
            {
                var winElement = windowEnumerator.FindWindowByInfo(win);
                if (winElement == null) continue;
                allElements.AddRange(UiaTreeWalker.Walk(winElement, 10, roleFilter, req.Label));
            }

            var result = UiaQueryResolver.Resolve(allElements, roleFilter, req.Label, req.Id, req.Index);

            return result.Result switch
            {
                QueryResult.NotFound => Results.Json(
                    new ErrorResponse { Error = "No element found matching query" },
                    statusCode: 400),
                QueryResult.Multiple => Results.Json(
                    new ErrorResponse
                    {
                        Error = "Multiple elements matched",
                        Details = string.Join("\n", result.Matches!.Select(DescribeElement)),
                    },
                    statusCode: 400),
                _ => BuildInspectResponse(result.Element!, windowEnumerator),
            };
        });

        app.MapPost("/press", (ElementQuery req) =>
            ResolveAndAct(req, "press", UiaActionPerformer.Press, windowEnumerator));

        app.MapPost("/set-value", (SetValueRequest req) =>
        {
            var query = new ElementQuery
            {
                Role = req.Role, Label = req.Label, Window = req.Window, Id = req.Id, Index = req.Index,
            };
            return ResolveAndAct(query, "set-value",
                el => UiaActionPerformer.SetValue(el, req.Value), windowEnumerator);
        });

        app.MapPost("/focus", (ElementQuery req) =>
            ResolveAndAct(req, "focus", UiaActionPerformer.Focus, windowEnumerator));

        app.MapPost("/show-menu", (ElementQuery req) =>
            ResolveAndAct(req, "show-menu", UiaActionPerformer.ShowMenu, windowEnumerator));

        MapWindowManagementEndpoints(app, windowEnumerator);

        app.MapPost("/wait", async (WaitRequest req) =>
        {
            var timeout = req.Timeout ?? 10;
            var deadline = DateTime.UtcNow.AddSeconds(timeout);

            while (DateTime.UtcNow < deadline)
            {
                var windows = windowEnumerator.EnumerateWindows();
                if (req.Window != null)
                    windows = windows.Where(w => WindowEnumerator.WindowInfoMatches(w, req.Window)).ToList();

                if (windows.Count > 0)
                    return Results.Json(new ActionResponse { Success = true, Message = "Accessibility ready" });

                await Task.Delay(500);
            }

            return Results.Json(new ActionResponse { Success = false, Message = "Timed out waiting for accessibility" });
        });
    }

    private static void MapWindowManagementEndpoints(WebApplication app, WindowEnumerator windowEnumerator)
    {
        app.MapPost("/window-focus", (WindowTarget req) =>
        {
            var winElement = windowEnumerator.FindWindowElement(req.Window);
            if (winElement == null)
                return Results.Json(new ErrorResponse { Error = $"No window matching '{req.Window}'" }, statusCode: 400);

            try
            {
                winElement.Focus();
                if (winElement.Patterns.Window.IsSupported)
                    winElement.Patterns.Window.Pattern.SetWindowVisualState(FlaUI.Core.Definitions.WindowVisualState.Normal);
                return Results.Json(new ActionResponse { Success = true, Message = "Window focused successfully" });
            }
            catch (Exception ex)
            {
                return Results.Json(new ActionResponse { Success = false, Message = $"window-focus failed: {ex.Message}" });
            }
        });

        app.MapPost("/window-resize", (WindowResizeRequest req) =>
        {
            var winElement = windowEnumerator.FindWindowElement(req.Window);
            if (winElement == null)
                return Results.Json(new ErrorResponse { Error = $"No window matching '{req.Window}'" }, statusCode: 400);

            try
            {
                if (winElement.Patterns.Transform.IsSupported)
                {
                    winElement.Patterns.Transform.Pattern.Resize(req.Width, req.Height);
                    return Results.Json(new ActionResponse
                    {
                        Success = true,
                        Message = $"Window resized to {req.Width}\u00d7{req.Height}",
                    });
                }
                return Results.Json(new ActionResponse
                {
                    Success = false,
                    Message = "Window does not support resize",
                });
            }
            catch (Exception ex)
            {
                return Results.Json(new ActionResponse { Success = false, Message = $"window-resize failed: {ex.Message}" });
            }
        });

        app.MapPost("/window-move", (WindowMoveRequest req) =>
        {
            var winElement = windowEnumerator.FindWindowElement(req.Window);
            if (winElement == null)
                return Results.Json(new ErrorResponse { Error = $"No window matching '{req.Window}'" }, statusCode: 400);

            try
            {
                if (winElement.Patterns.Transform.IsSupported)
                {
                    winElement.Patterns.Transform.Pattern.Move(req.X, req.Y);
                    return Results.Json(new ActionResponse
                    {
                        Success = true,
                        Message = $"Window moved to ({req.X}, {req.Y})",
                    });
                }
                return Results.Json(new ActionResponse
                {
                    Success = false,
                    Message = "Window does not support move",
                });
            }
            catch (Exception ex)
            {
                return Results.Json(new ActionResponse { Success = false, Message = $"window-move failed: {ex.Message}" });
            }
        });

        app.MapPost("/window-close", (WindowTarget req) =>
        {
            var winElement = windowEnumerator.FindWindowElement(req.Window);
            if (winElement == null)
                return Results.Json(new ErrorResponse { Error = $"No window matching '{req.Window}'" }, statusCode: 400);

            try
            {
                if (winElement.Patterns.Window.IsSupported)
                {
                    winElement.Patterns.Window.Pattern.Close();
                    return Results.Json(new ActionResponse { Success = true, Message = "Window closed successfully" });
                }
                return Results.Json(new ActionResponse { Success = false, Message = "Window does not support close" });
            }
            catch (Exception ex)
            {
                return Results.Json(new ActionResponse { Success = false, Message = $"window-close failed: {ex.Message}" });
            }
        });

        app.MapPost("/window-minimize", (WindowTarget req) =>
        {
            var winElement = windowEnumerator.FindWindowElement(req.Window);
            if (winElement == null)
                return Results.Json(new ErrorResponse { Error = $"No window matching '{req.Window}'" }, statusCode: 400);

            try
            {
                if (winElement.Patterns.Window.IsSupported)
                {
                    winElement.Patterns.Window.Pattern.SetWindowVisualState(FlaUI.Core.Definitions.WindowVisualState.Minimized);
                    return Results.Json(new ActionResponse { Success = true, Message = "Window minimized successfully" });
                }
                return Results.Json(new ActionResponse { Success = false, Message = "Window does not support minimize" });
            }
            catch (Exception ex)
            {
                return Results.Json(new ActionResponse { Success = false, Message = $"window-minimize failed: {ex.Message}" });
            }
        });
    }

    private static IResult ResolveAndAct(
        ElementQuery query,
        string actionName,
        Action<AutomationElement> perform,
        WindowEnumerator windowEnumerator)
    {
        UnifiedRole? roleFilter = query.Role != null ? RoleMapper.MapFromString(query.Role) : null;

        var windows = windowEnumerator.EnumerateWindows();
        if (query.Window != null)
            windows = windows.Where(w => WindowEnumerator.WindowInfoMatches(w, query.Window)).ToList();

        if (windows.Count == 0)
            return Results.Json(new ErrorResponse { Error = "No matching windows found" }, statusCode: 400);

        var allElements = new List<ElementInfo>();
        foreach (var win in windows)
        {
            var winElement = windowEnumerator.FindWindowByInfo(win);
            if (winElement == null) continue;
            allElements.AddRange(UiaTreeWalker.Walk(winElement, 10, roleFilter, query.Label));
        }

        var result = UiaQueryResolver.Resolve(allElements, roleFilter, query.Label, query.Id, query.Index);

        switch (result.Result)
        {
            case QueryResult.NotFound:
                return Results.Json(new ErrorResponse { Error = "No element found matching query" }, statusCode: 400);
            case QueryResult.Multiple:
                return Results.Json(new ErrorResponse
                {
                    Error = "Multiple elements matched \u2014 refine your query or use index",
                    Details = string.Join("\n", result.Matches!.Select(DescribeElement)),
                }, statusCode: 400);
        }

        var liveElement = windowEnumerator.FindLiveElement(result.Element!);
        if (liveElement == null)
            return Results.Json(new ErrorResponse
            {
                Error = "Element found in snapshot but could not locate live UIA element",
            }, statusCode: 400);

        try
        {
            perform(liveElement);
            return Results.Json(new ActionResponse
            {
                Success = true,
                Message = $"{actionName} performed successfully",
            });
        }
        catch (Exception ex)
        {
            return Results.Json(new ActionResponse
            {
                Success = false,
                Message = $"{actionName} failed: {ex.Message}",
            });
        }
    }

    private static IResult BuildInspectResponse(ElementInfo element, WindowEnumerator windowEnumerator)
    {
        var liveElement = windowEnumerator.FindLiveElement(element);

        double? boundsX = null, boundsY = null, boundsW = null, boundsH = null;
        if (liveElement != null)
        {
            try
            {
                var bounds = liveElement.BoundingRectangle;
                if (!bounds.IsEmpty)
                {
                    boundsX = bounds.X;
                    boundsY = bounds.Y;
                    boundsW = bounds.Width;
                    boundsH = bounds.Height;
                }
            }
            catch { }
        }

        return Results.Json(new InspectResponse
        {
            Element = element,
            BoundsX = boundsX,
            BoundsY = boundsY,
            BoundsWidth = boundsW,
            BoundsHeight = boundsH,
        });
    }

    private static List<ElementInfo> FilterInteractive(List<ElementInfo> elements)
    {
        return elements
            .Select(FilterInteractiveElement)
            .Where(e => e != null)
            .Select(e => e!)
            .ToList();
    }

    private static ElementInfo? FilterInteractiveElement(ElementInfo element)
    {
        var filteredChildren = element.Children != null ? FilterInteractive(element.Children) : [];
        var selfInteractive = IsInteractive(element);
        if (!selfInteractive && filteredChildren.Count == 0) return null;

        return new ElementInfo
        {
            Role = element.Role,
            Label = element.Label,
            Value = element.Value,
            Description = element.Description,
            Id = element.Id,
            Enabled = element.Enabled,
            Focused = element.Focused,
            PositionX = element.PositionX,
            PositionY = element.PositionY,
            SizeWidth = element.SizeWidth,
            SizeHeight = element.SizeHeight,
            ChildCount = element.ChildCount,
            Actions = element.Actions,
            PlatformRole = element.PlatformRole,
            Children = filteredChildren.Count > 0 ? filteredChildren : null,
        };
    }

    private static bool IsInteractive(ElementInfo element)
    {
        if (element.Actions.Count > 0) return true;
        if (element.Focused) return true;
        return element.Role is UnifiedRole.Button or UnifiedRole.Checkbox or UnifiedRole.Radio
            or UnifiedRole.Textfield or UnifiedRole.EditableText or UnifiedRole.Slider
            or UnifiedRole.ComboBox or UnifiedRole.Switch or UnifiedRole.Link
            or UnifiedRole.MenuItem or UnifiedRole.Tab or UnifiedRole.DisclosureTriangle
            or UnifiedRole.ColorWell or UnifiedRole.DatePicker or UnifiedRole.SpinButton;
    }

    private static List<ElementInfo> FilterLayout(List<ElementInfo> elements)
    {
        return elements
            .Select(FilterLayoutElement)
            .Where(e => e != null)
            .Select(e => e!)
            .ToList();
    }

    private static ElementInfo? FilterLayoutElement(ElementInfo element)
    {
        var filteredChildren = element.Children != null ? FilterLayout(element.Children) : [];
        var hasGeometry = element.PositionX.HasValue && element.SizeWidth.HasValue;
        if (!hasGeometry && filteredChildren.Count == 0) return null;

        return new ElementInfo
        {
            Role = element.Role,
            Label = element.Label,
            Value = element.Value,
            Description = element.Description,
            Id = element.Id,
            Enabled = element.Enabled,
            Focused = element.Focused,
            PositionX = element.PositionX,
            PositionY = element.PositionY,
            SizeWidth = element.SizeWidth,
            SizeHeight = element.SizeHeight,
            ChildCount = element.ChildCount,
            Actions = element.Actions,
            PlatformRole = element.PlatformRole,
            Children = filteredChildren.Count > 0 ? filteredChildren : null,
        };
    }

    private static string DescribeElement(ElementInfo info)
    {
        var parts = new List<string> { info.Role.ToString() };
        if (info.Label != null) parts.Add($"label={info.Label}");
        if (info.Id != null) parts.Add($"id={info.Id}");
        if (info.PositionX.HasValue && info.PositionY.HasValue)
            parts.Add($"pos=({(int)info.PositionX.Value},{(int)info.PositionY.Value})");
        return string.Join(" ", parts);
    }
}
