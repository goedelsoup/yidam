### A staged run and `vault gc` leave another user's running process alone

**A process this user may not signal now reads as running, not exited (#1703).**

Before, `kill -0` failing for any reason read as "exited".
It also fails for a live process another user owns.
So a staged run's sweep removed that user's scratch worktree or index copy under the git directory.
`yidam vault gc` listed that user's `.part` file as left by an interrupted write, and `--yes` deleted it.
Now only "No such process" reads as exited.
A `kill` that cannot run, or reports anything else, reads as running, and its files are left alone.
A pid from another PID namespace still reads as exited, so two containers should not share one git directory.
