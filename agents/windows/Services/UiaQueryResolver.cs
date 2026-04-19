using TestAnywareAgent.Models;

namespace TestAnywareAgent.Services;

public enum QueryResult
{
    Found,
    NotFound,
    Multiple,
}

public sealed class ResolvedQuery
{
    public QueryResult Result { get; init; }
    public ElementInfo? Element { get; init; }
    public List<ElementInfo>? Matches { get; init; }
}

public static class UiaQueryResolver
{
    public static ResolvedQuery Resolve(
        List<ElementInfo> elements,
        UnifiedRole? role,
        string? label,
        string? id,
        int? index)
    {
        var candidates = CollectMatching(elements, role, label, id);

        if (candidates.Count == 0)
            return new ResolvedQuery { Result = QueryResult.NotFound };

        if (index.HasValue)
        {
            if (index.Value >= 0 && index.Value < candidates.Count)
                return new ResolvedQuery { Result = QueryResult.Found, Element = candidates[index.Value] };
            return new ResolvedQuery { Result = QueryResult.NotFound };
        }

        if (candidates.Count == 1)
            return new ResolvedQuery { Result = QueryResult.Found, Element = candidates[0] };

        return new ResolvedQuery { Result = QueryResult.Multiple, Matches = candidates };
    }

    private static List<ElementInfo> CollectMatching(
        List<ElementInfo> elements,
        UnifiedRole? role,
        string? label,
        string? id)
    {
        var results = new List<ElementInfo>();

        foreach (var element in elements)
        {
            var matchesRole = role == null || element.Role == role;
            var matchesLabel = label == null ||
                (element.Label?.Contains(label, StringComparison.OrdinalIgnoreCase) ?? false);
            var matchesId = id == null ||
                string.Equals(element.Id, id, StringComparison.OrdinalIgnoreCase);

            if (matchesRole && matchesLabel && matchesId)
                results.Add(element);

            if (element.Children != null)
                results.AddRange(CollectMatching(element.Children, role, label, id));
        }

        return results;
    }
}
