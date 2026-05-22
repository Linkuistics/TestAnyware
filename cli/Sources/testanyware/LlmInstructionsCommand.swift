import ArgumentParser

/// Prints the full LLM usage guide, embedded at build time from
/// `cli/LLM_INSTRUCTIONS.md` (see `LLMInstructions.generated.swift`).
struct LlmInstructionsCommand: ParsableCommand {
    static let configuration = CommandConfiguration(
        commandName: "llm-instructions",
        abstract: "Print the full LLM usage guide for this tool (LLM agents: read this first)",
        discussion: """
            Emits the complete TestAnyware usage guide as plain text on \
            stdout: the command surface, how to connect to a VM, end-to-end \
            workflows, and common mistakes. Written for an LLM agent driving \
            the CLI — read it, or prepend it to the agent's context.

            EXAMPLES:
              testanyware llm-instructions
              testanyware llm-instructions > testanyware-guide.txt
            """
    )

    func run() throws {
        print(LLMInstructions.text, terminator: "")
    }
}
