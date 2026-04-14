# Security Notes for Launch Scripts

## Account Credentials Files

The launch scripts require account credentials to automate EverQuest client login.

### Plaintext Credential Files (accounts_group*.txt)

These files store EverQuest account usernames and passwords. **They must never be committed to git.**

- **Files**: `accounts_group1.txt`, `accounts_group2.txt`, etc.
- **Status**: Listed in `.gitignore` to prevent accidental commits
- **Setup**: Copy `accounts_group1.txt.example` to `accounts_group1.txt` and edit with real credentials
- **Format**: Space-separated on each line: `<account_name> <password>`

### Pre-Commit Hook Protection

The repository includes a pre-commit hook (`.git/hooks/pre-commit`) that:
1. Checks for any `scripts/accounts_*.txt` files staged for commit (excluding `.example` files)
2. Blocks the commit if found and displays instructions to unstage them
3. Runs gitleaks to detect other potential secrets

If you accidentally stage a credential file, unstage it with:
```bash
git reset HEAD scripts/accounts_group1.txt
```

### Machine Compromise Risk

If the machine running these scripts is compromised:
- All account passwords in the plaintext credential files are immediately exposed
- The injected DLL has read access to these files

### Alternative: Encrypted Credential Store

The TextQuest codebase includes an encrypted credential store in `textquest/src/credentials/` with:
- AES-256-GCM encryption
- Argon2id key derivation
- Future launch scripts should use `textquest.exe autologin --account <name>` with the encrypted store instead of reading plaintext files

### Recommendations

1. Use strong, unique passwords for all bot accounts
2. Keep the machine hosting these scripts secure and isolated
3. Rotate credentials periodically
4. Migrate launch scripts to use the encrypted credential store when available
5. Never share credential files or expose them outside your secure environment
