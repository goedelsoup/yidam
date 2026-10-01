# A GitHub App key that signs for nothing

`private-key.pem` and `private-key.pkcs8.pem` are one 2048-bit RSA key, generated for these
tests and registered with no GitHub App. The first is PKCS#1, the form GitHub hands out when an
App's private key is generated; the second is the same key as PKCS#8, which `openssl` writes by
default. The lander reads both (#1233).

Committed rather than generated per run because `ring` signs with an RSA key and cannot make
one. A JWT signed with it is refused by every GitHub instance, since no App holds its public half.
