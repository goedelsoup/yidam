/**
 * The text typed into a terminal to run the binary the extension resolved.
 *
 * Pure, so a test can reach it without an editor. Every other call goes through
 * `state.resolution.command`; a terminal is the one place that command is typed rather than
 * spawned, so it is the one place that has to be quoted.
 */

/**
 * A path is typed unchanged when it has no space, double-quoted when it does, and refused
 * when it holds anything a shell would read as syntax.
 *
 * The terminal's shell varies, and as with `isTerminalSafeRef` no single quoting rule is
 * right for all of them, so the allowlist is the characters a path legitimately holds
 * (letters, digits, `._/\:@+-` and a space) and everything else is refused rather than
 * escaped. Double quotes are the one form zsh, bash, fish, PowerShell and cmd all read as
 * "one word, nothing expanded" once `$`, a backtick, `"`, `'` and `%` are excluded.
 * PowerShell alone then needs `&` to call a quoted path instead of printing it.
 *
 * Returns `null` for a path that cannot be typed safely; the caller says so and does not run.
 */
export function terminalCommand(
  bin: string,
  args: readonly string[],
  shell: string,
): string | null {
  if (!/^[A-Za-z0-9._/\\:@+ -]+$/.test(bin) || bin.trim() !== bin) return null
  const word = bin.includes(' ') ? `"${bin}"` : bin
  const call = bin.includes(' ') && /(?:^|[\\/])(?:pwsh|powershell)(?:\.exe)?$/i.test(shell)
    ? `& ${word}`
    : word
  return [call, ...args].join(' ')
}
