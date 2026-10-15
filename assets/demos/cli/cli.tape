# i18n-le, the command line, driving the release build against input/.
#   cargo build --release in crate/, then from letools-site:
#   bun run record:demo ../i18n-le cli
# Runs in a temp copy, so nothing here is changed and no home path is shown.

Set Shell "bash"
Set FontSize 28
Set Width 1600
Set Height 1000
Set Padding 32
Set TypingSpeed 45ms
Set Theme "Catppuccin Mocha"

Hide
Type `D=$PWD; cd "$(mktemp -d /tmp/i18n-le.XXXX)" && cp -R "$D/input/." . && export PATH="$D/../../../crate/target/release:$PATH" && PS1="$ " && clear`
Enter
Type "cat package.json"
Enter
Sleep 2.5s
Show

Type "i18n-le locales > findings.jsonl"
Enter
Sleep 4s

Type "echo exit=$?"
Enter
Sleep 2.5s
