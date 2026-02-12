# Security Policy

## Reporting a Vulnerability

If you discover a security vulnerability in Tasker Core, please report it responsibly.

**Do not open a public issue for security vulnerabilities.**

Instead, email **pete.jc.taylor@hey.com** with:

- Description of the vulnerability
- Steps to reproduce
- Impact assessment (what can be exploited)
- Suggested fix (if you have one)

You should receive a response within 48 hours acknowledging receipt. We will work with you to understand the issue and coordinate a fix before any public disclosure.

## Supported Versions

| Version | Supported |
|---------|-----------|
| 0.1.x | Yes |

## Security Practices

Tasker Core follows these security practices:

- **Authentication**: API key and JWT authentication with configurable verification methods
- **Authorization**: Permission-based access control with handler-level security contexts
- **SQL injection prevention**: All database queries use SQLx compile-time verification
- **Dependency auditing**: Weekly Dependabot updates and `cargo audit` in CI
- **No unsafe code**: Minimal unsafe usage, all blocks documented with safety comments

## Disclosure Policy

- We will acknowledge receipt within 48 hours
- We will confirm the vulnerability and determine its impact
- We will release a fix and coordinate disclosure timing with the reporter
- We will credit reporters in the release notes (unless anonymity is requested)
