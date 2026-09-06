//! Verifying an elector's commits against the key its registry row binds — RFC-0012, as
//! amended 2026-09-04.
//!
//! **What a signature buys, and what it does not.** The RFC's original claim — that signing
//! makes a position "cryptographically attributable rather than a bare branch name" — was
//! retracted against measurement: 126 commits across the three elector branches of the one
//! repository running a sangha carry one git author, and under a single operator one key
//! attests the operator while three keys attest a convention. What survives is integrity (the
//! commit is the bytes the key-holder produced) and third-party verification (a reader outside
//! the repository can check, from the registry alone, that a seat's commits verify against the
//! key that seat declares). This module implements the second; nothing here claims the first.
//!
//! **The registry is the trust root.** `git verify-commit` reads `gpg.ssh.allowedSignersFile`,
//! and that file is generated *here*, at verification time, from `electors.md` — one principal
//! per keyed seat, never committed, so the registry and the trust anchor cannot drift. A key
//! absent from the registry verifies nothing. That is the whole difference between this and
//! the `Key (fpr)` column the RFC first proposed: a fingerprint is a record nothing consults.
//!
//! **Why the standing lint is the venue.** Verification consumes only public material — the
//! registry and the commit objects — so no workflow needs a signing secret, and nothing here
//! touches git configuration: the allowed-signers path is passed as `git -c` for the duration
//! of one read. `tag.gpgsign` is not set, read, or implied. What makes the check conditional
//! is the registry's own declaration, which is repository content, so the check returns the
//! same answer in every venue: vacuous in a corpus with no keyed seats — which is every corpus
//! today, collective mode being opt-in — and armed by the commit that lands a key.

use std::path::Path;

use super::model::{Check, Severity, Violation};
use crate::cmd::sangha::Elector;

/// What the registry says about a seat, reduced to what verification needs.
///
/// Taken as data rather than read off disk, so every judgement below is testable without a
/// repository, a keypair, or an `ssh-keygen`.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Seat {
    /// `ma/<name>` — the principal, and the seat's identity everywhere else in the record.
    pub branch: String,
    /// The `Key` cell, verbatim.
    pub key: String,
}

/// The seats whose rows bind a key. Everything in this module is empty without them.
pub(crate) fn keyed_seats(electors: &[Elector]) -> Vec<Seat> {
    electors
        .iter()
        .filter(|e| !e.key.is_empty())
        .map(|e| Seat {
            branch: e.branch.clone(),
            key: e.key.clone(),
        })
        .collect()
}

/// Whether a cell holds an SSH public key rather than something that merely mentions one.
///
/// Deliberately shallow — it separates a key from a *fingerprint*, which is the one confusion
/// this column invites, since `SHA256:…` is what the RFC's first table printed and what a
/// `git log --format=%GK` prints. A fingerprint in this column would generate an
/// allowed-signers file that verifies nothing while looking exactly like one that does.
fn is_public_key(cell: &str) -> bool {
    let algo = cell.split_whitespace().next().unwrap_or_default();
    (algo.starts_with("ssh-") || algo.starts_with("ecdsa-") || algo.starts_with("sk-"))
        && cell.split_whitespace().count() >= 2
}

/// The allowed-signers file `electors.md` generates.
///
/// One principal per keyed seat, the principal being the seat's `ma/*` branch: it cannot
/// contain a space (git forbids it) and it is the name `synthesized-by:` and `tips:` already
/// use, so the file names seats in the vocabulary the record does. The committer's email is
/// deliberately *not* the principal — `ssh-keygen -Y find-principals` matches on the key, so
/// a repository whose seats share one git identity (the measured case: all of them) still
/// gets a per-seat answer.
///
/// `namespaces="git"` scopes each entry to git's signing namespace, so a key the registry
/// binds for commits cannot be borrowed to verify an `ssh-keygen -Y` signature made for
/// anything else.
///
/// Rows whose key cell is not a public key are omitted rather than written through: an
/// allowed-signers file with a malformed line is one `ssh-keygen` rejects wholesale, which
/// would turn one bad cell into every seat failing. The omission is itself a finding, in
/// [`elector_signature_unverified`].
pub(crate) fn allowed_signers(seats: &[Seat]) -> String {
    let mut out = String::new();
    for s in seats.iter().filter(|s| is_public_key(&s.key)) {
        out.push_str(&format!("{} namespaces=\"git\" {}\n", s.branch, s.key));
    }
    out
}

/// Whether the registry binds a *distinct* signing key to every seat.
///
/// This is the escalation condition `resolution-executor-unrecorded` was written with, made
/// decidable: "when `electors.md` binds a distinct signing key per seat, the executor is
/// recoverable from the commit and a missing field is a choice rather than an inheritance."
///
/// Three ways it is false, and each is a real state rather than an oversight:
///
/// - **No seats at all.** Collective mode is opt-in and most repositories are here. Nothing
///   to escalate about.
/// - **A seat with no key.** Its commits are unverifiable by construction, so the executor is
///   *not* recoverable from the commit for that seat, and a record it wrote inherits the same
///   excuse every pre-field record has.
/// - **A key shared between seats.** Legitimate — RFC-0012 says plainly that one key under
///   one operator attests the operator — and it is exactly the case where the commit cannot
///   tell one seat from another. Sharing a key is allowed; it just does not arm the gate.
pub(crate) fn binds_distinct_key_per_seat(electors: &[Elector]) -> bool {
    if electors.is_empty() || electors.iter().any(|e| e.key.is_empty()) {
        return false;
    }
    let mut keys: Vec<&str> = electors.iter().map(|e| e.key.as_str()).collect();
    keys.sort_unstable();
    keys.dedup();
    keys.len() == electors.len()
}

/// What verification found for one seat.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Verdict {
    /// The tip is signed by the key this seat's row binds.
    Verified,
    /// The row binds a key and the tip carries no signature.
    Unsigned,
    /// A good signature by a key the registry does not carry.
    UnknownKey,
    /// A good signature — by the key another seat's row binds.
    OtherSeat(String),
    /// The `Key` cell is not an SSH public key, so no allowed-signers entry could be made.
    NotAKey,
    /// The registry registers the branch and git has no such ref. Reported by the sangha
    /// report already; carried here so the check can stay silent about it rather than
    /// inventing a signature failure out of a missing branch.
    NoBranch,
    /// git answered something this cannot read — a bad, expired or revoked signature, or an
    /// environment with no `ssh-keygen`. The raw status code, so the finding names what
    /// happened rather than guessing why.
    Undecided(String),
}

/// One seat's verification outcome.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Attestation {
    pub branch: String,
    pub verdict: Verdict,
}

/// Read `%G?` and `%GS` for a ref, under a generated allowed-signers file.
fn signature_of(root: &Path, allowed: &Path, git_ref: &str) -> Option<(String, String)> {
    let out = std::process::Command::new("git")
        .current_dir(root)
        .arg("-c")
        .arg(format!(
            "gpg.ssh.allowedSignersFile={}",
            allowed.to_string_lossy()
        ))
        .args(["log", "-1", "--format=%G?%x00%GS", git_ref])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8(out.stdout).ok()?;
    let (status, signer) = text.trim_end_matches('\n').split_once('\0')?;
    Some((status.to_string(), signer.to_string()))
}

/// Where the generated allowed-signers file lives while it is being read.
///
/// The system temp directory, never the repository: RFC-0012 says the file is generated at
/// verification time and never committed, and `yidam lint` writing into the tree it is
/// linting would be a check with a side effect. Removed on drop, including when a check
/// panics part-way through.
struct Generated(std::path::PathBuf);

impl Generated {
    fn write(content: &str) -> Option<Self> {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or_default();
        let path = std::env::temp_dir().join(format!(
            "yidam-allowed-signers-{}-{nonce}",
            std::process::id()
        ));
        std::fs::write(&path, content).ok()?;
        Some(Generated(path))
    }
}

impl Drop for Generated {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

/// Verify each keyed seat's branch tip against the registry-derived allowed signers.
///
/// **The tip, and not the branch's history.** A key bound today cannot retroactively sign what
/// was committed before it, so verifying every commit on a `ma/*` branch would report the
/// whole of its history the day the registry gains a key — a debt that cannot be paid, which
/// is how a gate gets switched off. The same reasoning `resolution-executor-unrecorded`
/// records for its own severity. What the tip answers is the question a reader actually has
/// when they read a position: *is this, now, the seat it says it is.*
pub(crate) fn attest(root: &Path, electors: &[Elector]) -> Vec<Attestation> {
    let seats = keyed_seats(electors);
    if seats.is_empty() {
        // The common case, and it costs nothing: no temp file, no `git`, no `ssh-keygen`.
        return Vec::new();
    }
    let refs: std::collections::HashMap<String, String> = crate::git::phase_refs(root)
        .into_iter()
        .map(|r| (r.name, r.git_ref))
        .collect();
    let Some(allowed) = Generated::write(&allowed_signers(&seats)) else {
        return seats
            .iter()
            .map(|s| Attestation {
                branch: s.branch.clone(),
                verdict: Verdict::Undecided("the allowed-signers file could not be written".into()),
            })
            .collect();
    };

    // Which seat a principal names, and the key it binds — so a signature that verifies as
    // another principal can be reported as *that seat's key*, which is the finding, rather
    // than as an unknown one.
    let by_branch: std::collections::HashMap<&str, &str> = seats
        .iter()
        .map(|s| (s.branch.as_str(), s.key.as_str()))
        .collect();

    seats
        .iter()
        .map(|s| {
            let verdict = if !is_public_key(&s.key) {
                Verdict::NotAKey
            } else if let Some(git_ref) = refs.get(&s.branch) {
                match signature_of(root, &allowed.0, git_ref) {
                    None => Verdict::Undecided("git could not read the branch tip".into()),
                    Some((status, signer)) => match status.as_str() {
                        "N" => Verdict::Unsigned,
                        // Good, and matched a principal. Whose?
                        "G" if signer == s.branch => Verdict::Verified,
                        // A seat sharing this seat's key is this seat's key: `find-principals`
                        // matches on the key and returns whichever principal it reaches first,
                        // so two seats with one key would otherwise report each other.
                        "G" if by_branch.get(signer.as_str()) == Some(&s.key.as_str()) => {
                            Verdict::Verified
                        }
                        "G" => Verdict::OtherSeat(signer),
                        // Good signature, no principal matched: a key the registry does not
                        // carry. This is the arm that makes the registry a trust root.
                        "U" => Verdict::UnknownKey,
                        other => Verdict::Undecided(format!("git reports `{other}`")),
                    },
                }
            } else {
                Verdict::NoBranch
            };
            Attestation {
                branch: s.branch.clone(),
                verdict,
            }
        })
        .collect()
}

/// A seat whose tip does not verify against the key its own registry row binds.
pub(crate) fn elector_signature_unverified(attestations: &[Attestation]) -> Check {
    let violations = attestations
        .iter()
        .filter_map(|a| {
            let detail = match &a.verdict {
                Verdict::Verified | Verdict::NoBranch => return None,
                Verdict::Unsigned => "this row binds a signing key and the branch tip carries \
                                      no signature — the key attests nothing it has not signed"
                    .to_string(),
                Verdict::UnknownKey => "the tip is signed by a key `electors.md` does not \
                                        carry — the registry is the trust root, so a key it \
                                        does not bind verifies nothing"
                    .to_string(),
                Verdict::OtherSeat(other) => format!(
                    "the tip is signed by the key `electors.md` binds to `{other}`, not by \
                     this seat's"
                ),
                Verdict::NotAKey => "the `Key` cell is not an SSH public key — an \
                                     allowed-signers file needs the key itself, and a \
                                     fingerprint records one without verifying anything"
                    .to_string(),
                Verdict::Undecided(why) => {
                    format!("the tip's signature could not be checked here: {why}")
                }
            };
            Some(Violation::new(a.branch.clone(), detail))
        })
        .collect();
    Check::new(
        "elector-signature-unverified",
        "Elector branch tip does not verify against the key its row binds",
        Severity::Error,
        "RFC-0012, amended: a signature buys integrity and third-party verification, and it \
         is worth having for those and claimed for nothing more — under one operator it does \
         not distinguish seats from one another, and `electors.md` says so. This gates rather \
         than warns because the repository decided it: the check is vacuous until a row binds \
         a key, and binding one is a declaration that this seat's commits verify against it. \
         The allowed-signers file is generated from the registry at verification time and \
         never committed, so the trust anchor cannot drift from the record, and a key the \
         registry does not carry verifies nothing.",
        violations,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn elector(branch: &str, key: &str) -> Elector {
        Elector {
            name: branch.trim_start_matches("ma/").to_string(),
            branch: branch.to_string(),
            role: String::new(),
            kind: "agent".to_string(),
            model: String::new(),
            version: String::new(),
            config: String::new(),
            key: key.to_string(),
            branch_present: true,
        }
    }

    const K1: &str = "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIAAAA1";
    const K2: &str = "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIAAAA2";

    /// **Collective mode is opt-in, and this is the state nearly every repository is in.**
    /// Every arm has to be silent here, and silent without touching git: a registry with no
    /// rows generates an empty trust root, verifies nothing, and escalates nothing.
    #[test]
    fn a_repository_with_no_electors_finds_nothing() {
        assert_eq!(allowed_signers(&keyed_seats(&[])), "");
        assert!(attest(Path::new("/nonexistent"), &[]).is_empty());
        assert!(elector_signature_unverified(&[]).passed());
        assert!(!binds_distinct_key_per_seat(&[]));
    }

    /// A registry written before RFC-0012 — three columns, no keys — is the same silence.
    #[test]
    fn a_registry_that_binds_no_key_finds_nothing() {
        let e = [elector("ma/auditor", ""), elector("ma/advocate", "")];
        assert!(keyed_seats(&e).is_empty());
        assert!(attest(Path::new("/nonexistent"), &e).is_empty());
        assert!(!binds_distinct_key_per_seat(&e));
    }

    #[test]
    fn the_registry_generates_one_principal_per_keyed_seat() {
        let e = [
            elector("ma/auditor", K1),
            elector("ma/advocate", K2),
            elector("ma/scribe", ""),
        ];
        assert_eq!(
            allowed_signers(&keyed_seats(&e)),
            format!("ma/auditor namespaces=\"git\" {K1}\nma/advocate namespaces=\"git\" {K2}\n")
        );
    }

    /// A fingerprint is what the RFC's first table printed and what `%GK` prints, so it is the
    /// cell a reader is most likely to paste in. It generates no entry — an allowed-signers
    /// file built from fingerprints verifies nothing while looking like one that does.
    #[test]
    fn a_fingerprint_is_not_a_key() {
        let e = [elector("ma/auditor", "SHA256:Y6WwqrkibLb+e0B0GvZTB5GBhTsm")];
        assert_eq!(allowed_signers(&keyed_seats(&e)), "");
        assert!(!is_public_key("SHA256:Y6WwqrkibLb"));
        assert!(is_public_key(K1));
    }

    #[test]
    fn the_escalation_needs_every_seat_keyed_and_the_keys_distinct() {
        assert!(binds_distinct_key_per_seat(&[
            elector("ma/auditor", K1),
            elector("ma/advocate", K2),
        ]));
        // One seat unkeyed: its commits are unverifiable, so the executor is not recoverable
        // from them and the escalation's premise is false.
        assert!(!binds_distinct_key_per_seat(&[
            elector("ma/auditor", K1),
            elector("ma/advocate", ""),
        ]));
        // One key across two seats — legitimate, and exactly the case the amendment says
        // distinguishes nothing. Allowed, and it does not arm the gate.
        assert!(!binds_distinct_key_per_seat(&[
            elector("ma/auditor", K1),
            elector("ma/advocate", K1),
        ]));
    }

    fn verdicts(v: &[Verdict]) -> Check {
        let a: Vec<Attestation> = v
            .iter()
            .enumerate()
            .map(|(i, verdict)| Attestation {
                branch: format!("ma/seat{i}"),
                verdict: verdict.clone(),
            })
            .collect();
        elector_signature_unverified(&a)
    }

    #[test]
    fn a_verified_tip_and_a_missing_branch_are_not_findings() {
        assert!(verdicts(&[Verdict::Verified, Verdict::NoBranch]).passed());
    }

    /// The four states that are findings, and each says which one it is. A check that
    /// reported "unverified" for all of them would leave the reader unable to tell a seat
    /// that never signed from one signed by somebody else's key.
    #[test]
    fn each_failure_names_what_happened_pure() {
        let c = verdicts(&[
            Verdict::Unsigned,
            Verdict::UnknownKey,
            Verdict::OtherSeat("ma/advocate".into()),
            Verdict::NotAKey,
            Verdict::Undecided("git reports `B`".into()),
        ]);
        assert_eq!(c.violations.len(), 5, "{c:#?}");
        assert!(c.violations[0].detail.contains("no signature"));
        assert!(c.violations[1].detail.contains("does not carry"));
        assert!(c.violations[2].detail.contains("ma/advocate"));
        assert!(c.violations[3].detail.contains("not an SSH public key"));
        assert!(c.violations[4].detail.contains("could not be checked"));
    }
}

/// Verification against real keys, real signatures and real `git`.
///
/// **This is the mutation the issue asked to write first**, and the reason is that every test
/// above is a test of judgement about an outcome git handed us — none of them can tell whether
/// the allowed-signers file this module generates is one `ssh-keygen` will accept, whether the
/// principal it writes is one `find-principals` will match, or whether `%GS` says what this
/// reads it as saying. A registry with no keys makes all of that vacuous, which is the state
/// of every repository today: without these, the check would go green by never running.
#[cfg(test)]
mod signing_tests {
    use super::*;
    use std::path::PathBuf;
    use std::process::Command;

    /// A keypair on disk. `ssh-keygen` writes the private key here and the public key beside
    /// it; git signs with the private one and this module's registry column carries the public.
    struct Key {
        private: PathBuf,
        public: String,
    }

    fn keygen(dir: &Path, name: &str) -> Key {
        let private = dir.join(name);
        let ok = Command::new("ssh-keygen")
            .args(["-q", "-t", "ed25519", "-N", "", "-C", name, "-f"])
            .arg(&private)
            .status()
            .expect("ssh-keygen is on PATH — git cannot make or check an SSH signature without it")
            .success();
        assert!(ok, "ssh-keygen failed");
        let public = std::fs::read_to_string(private.with_extension("pub")).unwrap();
        // `authorized_keys` form: algorithm and key, without the trailing comment.
        let public = public
            .split_whitespace()
            .take(2)
            .collect::<Vec<_>>()
            .join(" ");
        Key { private, public }
    }

    fn git(root: &Path, args: &[&str]) {
        let ok = Command::new("git")
            .current_dir(root)
            .args(args)
            .status()
            .unwrap()
            .success();
        assert!(ok, "git {args:?} failed");
    }

    /// A repository with one `ma/auditor` branch whose tip is signed by `signed_with`, or
    /// unsigned when that is `None`.
    ///
    /// `tag.gpgsign` and `commit.gpgsign` are both switched **off** in the scratch repository,
    /// for the reason `release_script.rs` and `template_pin.rs` do it: a developer's global
    /// configuration must not be able to change what this test is measuring. Signing here is
    /// per-commit and explicit, and nothing in this module reads or writes either setting.
    fn repo_signed_by(signed_with: Option<&Key>) -> (tempfile::TempDir, PathBuf) {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().to_path_buf();
        git(&root, &["init", "-q", "-b", "main"]);
        git(&root, &["config", "user.email", "operator@yidam.test"]);
        git(&root, &["config", "user.name", "Operator"]);
        git(&root, &["config", "commit.gpgsign", "false"]);
        git(&root, &["config", "tag.gpgsign", "false"]);
        git(&root, &["config", "gpg.format", "ssh"]);
        std::fs::write(root.join("a.md"), "seed\n").unwrap();
        git(&root, &["add", "-A"]);
        git(&root, &["commit", "-q", "-m", "seed"]);
        git(&root, &["switch", "-q", "-c", "ma/auditor"]);
        std::fs::write(root.join("position.md"), "a position\n").unwrap();
        git(&root, &["add", "-A"]);
        match signed_with {
            Some(k) => git(
                &root,
                &[
                    "-c",
                    &format!("user.signingkey={}", k.private.display()),
                    "commit",
                    "-q",
                    "-S",
                    "-m",
                    "open: a position",
                ],
            ),
            None => git(
                &root,
                &["commit", "-q", "--no-gpg-sign", "-m", "open: a position"],
            ),
        }
        (tmp, root)
    }

    fn verdict_for(root: &Path, electors: &[Elector]) -> Verdict {
        let a = attest(root, electors);
        assert_eq!(a.len(), 1, "{a:#?}");
        assert_eq!(a[0].branch, "ma/auditor");
        a[0].verdict.clone()
    }

    fn seat(branch: &str, key: &str) -> Elector {
        Elector {
            name: branch.trim_start_matches("ma/").to_string(),
            branch: branch.to_string(),
            role: String::new(),
            kind: "agent".to_string(),
            model: String::new(),
            version: String::new(),
            config: String::new(),
            key: key.to_string(),
            branch_present: true,
        }
    }

    /// The registry-derived allowed-signers file is one `ssh-keygen` accepts, and the
    /// principal it writes — the seat's `ma/*` branch — is one `find-principals` matches. The
    /// committer's email is `operator@yidam.test` and appears nowhere in the file, which is
    /// the point: the measured repository's seats share one git identity.
    #[test]
    fn a_tip_signed_by_the_key_its_row_binds_verifies() {
        let keys = tempfile::tempdir().unwrap();
        let k = keygen(keys.path(), "auditor");
        let (_tmp, root) = repo_signed_by(Some(&k));
        let e = [seat("ma/auditor", &k.public)];
        assert_eq!(verdict_for(&root, &e), Verdict::Verified);
        assert!(elector_signature_unverified(&attest(&root, &e)).passed());
    }

    /// **The issue's mutation.** The seat is registered with one key and its branch is signed
    /// with another; the check reports it, and it is `git` doing the rejecting rather than a
    /// string comparison here.
    #[test]
    fn a_tip_signed_by_a_key_the_registry_does_not_carry_is_reported() {
        let keys = tempfile::tempdir().unwrap();
        let registered = keygen(keys.path(), "registered");
        let actual = keygen(keys.path(), "actual");
        let (_tmp, root) = repo_signed_by(Some(&actual));
        let e = [seat("ma/auditor", &registered.public)];
        assert_eq!(verdict_for(&root, &e), Verdict::UnknownKey);
        let c = elector_signature_unverified(&attest(&root, &e));
        assert_eq!(c.violations.len(), 1, "{c:#?}");
        assert_eq!(c.violations[0].node, "ma/auditor");
        assert_eq!(c.severity, Severity::Error);
    }

    /// Three keys under one operator attest a convention about which key was used for which
    /// seat — and this is the check of that convention. The auditor's branch signed with the
    /// advocate's key verifies perfectly and is still wrong, and the finding names the seat
    /// whose key it was.
    #[test]
    fn a_tip_signed_by_another_seats_key_names_that_seat() {
        let keys = tempfile::tempdir().unwrap();
        let auditor = keygen(keys.path(), "auditor");
        let advocate = keygen(keys.path(), "advocate");
        let (_tmp, root) = repo_signed_by(Some(&advocate));
        let e = [
            seat("ma/auditor", &auditor.public),
            seat("ma/advocate", &advocate.public),
        ];
        let a = attest(&root, &e);
        assert_eq!(
            a[0].verdict,
            Verdict::OtherSeat("ma/advocate".to_string()),
            "{a:#?}"
        );
        // The advocate's own branch does not exist in this repository, which is a state the
        // sangha report already names and not a signature failure.
        assert_eq!(a[1].verdict, Verdict::NoBranch, "{a:#?}");
        let c = elector_signature_unverified(&a);
        assert_eq!(c.violations.len(), 1, "{c:#?}");
        assert!(c.violations[0].detail.contains("ma/advocate"), "{c:#?}");
    }

    /// One key across two seats is legitimate — it attests the operator — so a commit signed
    /// with it on either branch verifies. `find-principals` returns both principals and git
    /// reports whichever it reaches first, so a check comparing the principal alone would
    /// report one of the two seats as forged on every run.
    #[test]
    fn two_seats_sharing_one_key_both_verify() {
        let keys = tempfile::tempdir().unwrap();
        let shared = keygen(keys.path(), "operator");
        let (_tmp, root) = repo_signed_by(Some(&shared));
        let e = [
            seat("ma/advocate", &shared.public),
            seat("ma/auditor", &shared.public),
        ];
        let a = attest(&root, &e);
        let auditor = a.iter().find(|x| x.branch == "ma/auditor").unwrap();
        assert_eq!(auditor.verdict, Verdict::Verified, "{a:#?}");
        assert!(elector_signature_unverified(&a).passed(), "{a:#?}");
        // …and it does not arm the escalation, which is the other half of the same fact.
        assert!(!binds_distinct_key_per_seat(&e));
    }

    #[test]
    fn a_bound_key_over_an_unsigned_tip_is_reported() {
        let keys = tempfile::tempdir().unwrap();
        let k = keygen(keys.path(), "auditor");
        let (_tmp, root) = repo_signed_by(None);
        let e = [seat("ma/auditor", &k.public)];
        assert_eq!(verdict_for(&root, &e), Verdict::Unsigned);
    }

    /// A seat that binds no key is silent even where a signature exists to check — the
    /// registry's declaration is the whole condition, and an unregistered key is not one the
    /// registry can vouch for.
    #[test]
    fn a_seat_binding_no_key_is_not_verified_at_all() {
        let keys = tempfile::tempdir().unwrap();
        let k = keygen(keys.path(), "auditor");
        let (_tmp, root) = repo_signed_by(Some(&k));
        assert!(attest(&root, &[seat("ma/auditor", "")]).is_empty());
    }

    /// The generated file is not an artifact. It lives in the temp directory for the length of
    /// one read, and nothing is left in the repository being linted.
    #[test]
    fn verification_writes_nothing_into_the_repository() {
        let keys = tempfile::tempdir().unwrap();
        let k = keygen(keys.path(), "auditor");
        let (_tmp, root) = repo_signed_by(Some(&k));
        let before = tracked_and_untracked(&root);
        assert_eq!(
            verdict_for(&root, &[seat("ma/auditor", &k.public)]),
            Verdict::Verified
        );
        assert_eq!(
            before,
            tracked_and_untracked(&root),
            "verification left a file behind"
        );
    }

    fn tracked_and_untracked(root: &Path) -> String {
        let out = Command::new("git")
            .current_dir(root)
            .args(["status", "--porcelain", "--untracked-files=all"])
            .output()
            .unwrap();
        String::from_utf8(out.stdout).unwrap()
    }
}
