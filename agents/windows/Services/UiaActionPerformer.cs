using FlaUI.Core.AutomationElements;

namespace TestAnywareAgent.Services;

public static class UiaActionPerformer
{
    public static void Press(AutomationElement element)
    {
        if (element.Patterns.Invoke.IsSupported)
        {
            element.Patterns.Invoke.Pattern.Invoke();
            return;
        }
        if (element.Patterns.Toggle.IsSupported)
        {
            element.Patterns.Toggle.Pattern.Toggle();
            return;
        }
        if (element.Patterns.ExpandCollapse.IsSupported)
        {
            var pattern = element.Patterns.ExpandCollapse.Pattern;
            if (pattern.ExpandCollapseState.ValueOrDefault == FlaUI.Core.Definitions.ExpandCollapseState.Collapsed)
                pattern.Expand();
            else
                pattern.Collapse();
            return;
        }
        if (element.Patterns.SelectionItem.IsSupported)
        {
            element.Patterns.SelectionItem.Pattern.Select();
            return;
        }
        throw new InvalidOperationException("Element does not support press/invoke action");
    }

    public static void SetValue(AutomationElement element, string value)
    {
        if (element.Patterns.Value.IsSupported)
        {
            element.Patterns.Value.Pattern.SetValue(value);
            return;
        }
        throw new InvalidOperationException("Element does not support set-value");
    }

    public static void Focus(AutomationElement element)
    {
        element.Focus();
    }

    public static void ShowMenu(AutomationElement element)
    {
        if (element.Patterns.ExpandCollapse.IsSupported)
        {
            element.Patterns.ExpandCollapse.Pattern.Expand();
            return;
        }
        throw new InvalidOperationException("Element does not support show-menu/expand");
    }
}
