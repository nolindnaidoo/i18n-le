# i18n-le, the MCP server: an agent asks it about the files in ../cli/input.
#   from letools-site: bun run record:demo ../i18n-le mcp
# Claude Code on the logged-in account, in a temp copy, with only this tool's
# server loaded and its tools pre-approved. The config is written at run
# time, so no path is committed. PROMPT.md records the take that was kept.

Set Shell "bash"
Set FontSize 28
Set Width 1600
Set Height 1000
Set Padding 32
Set TypingSpeed 45ms
Set Theme "Catppuccin Mocha"

Hide
# The recording starts inside another Claude Code session; its markers would
# put that session's warnings on screen.
Type `unset CLAUDECODE CLAUDE_CODE_CHILD_SESSION CLAUDE_CODE_ENTRYPOINT CLAUDE_CODE_EXECPATH CLAUDE_CODE_MESSAGING_SOCKET CLAUDE_CODE_MESSAGING_TOKEN CLAUDE_CODE_SESSION_ATTENDED CLAUDE_CODE_SESSION_ID CLAUDE_EFFORT CLAUDE_PID`
Enter
Type `D=$PWD; cd "$(mktemp -d /tmp/i18n-le.XXXX)" && cp -R "$D/../cli/input/." . && printf '{"mcpServers":{"i18n-le":{"command":"node","args":["%s"]}}}' "$D/../../../mcp/server.js" > mcp.json && PS1="$ " && clear`
Enter
Type `clear && claude --mcp-config mcp.json --strict-mcp-config --allowedTools "mcp__i18n-le Read" --disallowedTools Bash WebFetch WebSearch Agent`
Enter
Sleep 6s
# The folder-trust prompt opens on "No, exit".
Down
Enter
Sleep 5s
Show
Sleep 1s
Type "Use i18n-le to audit the catalogues in locales/. One short line per problem."
Sleep 500ms
Enter
Sleep 24s
