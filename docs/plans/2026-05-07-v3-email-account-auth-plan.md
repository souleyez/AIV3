# V3 Email Account Auth Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Upgrade the current local-key visibility model into an email account system where users can sign in with email plus local key or email verification code, and datasets, robots, conversations, memory, reports, and static-page artifacts follow the user.

**Architecture:** V3 remains the identity, authorization, and data ownership source of truth in PostgreSQL. The existing local secret-binding model becomes an account-bound access factor instead of the only identity mechanism. Cloudflare Email Service is used as the verification-code delivery channel through a dedicated sender address; V3 stores verification challenges, sessions, user ownership, grants, and audit records.

**Tech Stack:** Rust `platform-api`, `contracts`, `domain-model`, `storage`, future `auth-scope`; Next.js web secret/account panel; PostgreSQL 17.9 migrations; Cloudflare Email Service Workers binding or REST API for OTP email delivery; optional Email Routing/Email Worker for inbound operational mailbox handling.

---

## Implementation Status

- Task 1-4 completed in first pass: auth domain/contracts, storage migration/repositories, OTP service, session cookie routes.
- Task 5 completed in first pass: ownership fields flow through datasets, documents, assistant runs, conversation memory, report plans, static-page drafts, and render outputs.
- Task 6 completed in compact first pass: existing sidebar key panel now supports email code login, email plus local-key login, current account status, logout, browser-local email cache, and API cookie pass-through.
- Task 7 completed in first pass: platform-api can use Cloudflare Email Service REST API when configured, otherwise it falls back to local logging sender; setup notes live in `docs/operations/cloudflare-email-auth-setup.md`.
- Remaining hardening: explicit key rotation/recovery UX, local-data claim flow, grants/team membership, robot ownership, audit expansion, and true encryption recovery semantics if required later.

---

## Source References

- Cloudflare Email Service overview: `https://developers.cloudflare.com/email-service/`
- Cloudflare Email Service send emails: `https://developers.cloudflare.com/email-service/get-started/send-emails/`
- Cloudflare Email Routing overview: `https://developers.cloudflare.com/email-routing/`
- Cloudflare Send emails from Workers: `https://developers.cloudflare.com/email-routing/email-workers/send-email-workers/`

Important Cloudflare boundary:

- Cloudflare's older Email Routing custom addresses are forwarding/routing addresses.
- Cloudflare Email Service now supports outbound transactional email through Workers binding or REST API when the domain is on Cloudflare DNS.
- Use Email Service for verification-code sending.
- Use Email Routing/Email Workers only if we need inbound handling such as replies, abuse reports, or operational mailbox routing.

## Product Requirements

- The existing key setting area gains an email input.
- A user can create or link an account with email plus local key.
- If a user forgets the local key, they can request a verification code by email.
- Future login supports either:
  - email + local key, or
  - email + verification code.
- Datasets, documents, robots, memory spaces, conversations, report plans, static-page drafts, and published artifacts are owned by a user.
- Public datasets remain visible to everyone.
- Private datasets and private robots are visible only to the owning user or explicitly granted users.
- Existing local-only data should migrate to a default local user until the user binds an email.
- The UI should stay simple: account email, current key, verification code, and active user status in the existing key panel.

## Non-Negotiable Security Rules

- Never email the raw local key.
- Never store the raw local key.
- Email verification proves control of an email address, not knowledge of the old key.
- If private data is truly encrypted by a key later, forgetting the key cannot magically decrypt it unless we also store an account recovery wrapping key. Do not pretend otherwise in UI.
- OTP codes are single-use, short-lived, hashed at rest, rate-limited, and audited.
- Login sessions use secure httpOnly cookies or signed server sessions. Do not store access tokens in browser localStorage.
- Browser localStorage may keep the active local key for the simple local workflow only, but server trust must come from a session or validated secret grant.
- Provider keys and Cloudflare tokens stay server-side.

## Identity Model

### Users

The current `users` table already exists. Extend it rather than replacing it.

Add fields over time:

```text
email_normalized
email_verified_at
auth_state
primary_secret_fingerprint
last_login_at
metadata
```

### User Sessions

Add durable sessions:

```text
user_sessions
  id
  tenant_id
  user_id
  device_fingerprint
  session_token_hash
  auth_method
  created_at
  last_seen_at
  expires_at
  revoked_at
```

### Verification Challenges

Add email OTP challenges:

```text
email_verification_challenges
  id
  tenant_id
  email_normalized
  purpose
  code_hash
  attempt_count
  max_attempts
  expires_at
  consumed_at
  metadata
  created_at
```

Supported purposes:

```text
account_create
login
recover_key
bind_email
rotate_key
```

### Ownership

Add user ownership to first-class product records.

First pass:

- `datasets.owner_user_id`
- `documents.owner_user_id`
- `assistant_runs.user_id`
- `conversation_memory_items.user_id`
- `report_plans.owner_user_id`
- `static_page_drafts.owner_user_id`
- `static_page_render_outputs.owner_user_id`

Later:

- robot definitions and bot instances.
- published report/static-page sharing grants.
- team/project membership.

## Recovery Semantics

There are two possible meanings of "forgot key"; the product must be honest.

### First Pass: Visibility Recovery

If current private dataset visibility is based on `secret_bindings` and server-side grants, email OTP can restore user access by proving account ownership and issuing fresh `secret_grants` for that user-owned data.

This supports:

- login by email OTP;
- active user/session restoration;
- access to user-owned private datasets and robots;
- setting a new local key for future uploads.

### Later: Cryptographic Recovery

If private data becomes encrypted with a user-held key, then email OTP alone cannot decrypt old content.

To support true key recovery later, add one of these:

- account recovery key wrapping;
- user-exported recovery code;
- hardware/passkey-backed wrapping;
- organization admin recovery policy.

Do not implement this in the first pass unless the storage encryption model is explicitly upgraded.

## Cloudflare Email Architecture

Preferred first version:

```text
V3 platform-api
  -> EmailSender trait
  -> Cloudflare Email Service REST API or Worker endpoint
  -> dedicated sender such as verify@<domain>
  -> user mailbox
```

Alternative when keeping Cloudflare logic isolated:

```text
V3 platform-api
  -> internal HTTPS call with service token
  -> Cloudflare Worker /v1/send-verification-code
  -> Email Service Workers binding
  -> user mailbox
```

Recommended:

- Keep V3 responsible for generating and storing OTP challenges.
- Let Cloudflare only send a rendered message.
- Do not let the Worker generate login state or decide users.
- Use queue/background sending if provider latency becomes visible.

Required secrets:

```text
CLOUDFLARE_EMAIL_ACCOUNT_ID
CLOUDFLARE_EMAIL_API_TOKEN
CLOUDFLARE_EMAIL_SENDER
CLOUDFLARE_EMAIL_WORKER_URL optional
CLOUDFLARE_EMAIL_WORKER_TOKEN optional
```

## API Surface

Add:

```text
POST /v1/auth/email/start
POST /v1/auth/email/verify
POST /v1/auth/key/login
POST /v1/auth/logout
GET  /v1/auth/session
POST /v1/auth/key/rotate
POST /v1/auth/email/bind
```

Request examples:

```json
{
  "email": "user@example.com",
  "purpose": "login"
}
```

```json
{
  "email": "user@example.com",
  "code": "123456",
  "purpose": "login",
  "device_fingerprint": "browser-local-device-id"
}
```

```json
{
  "email": "user@example.com",
  "local_key": "user-entered-key",
  "device_fingerprint": "browser-local-device-id"
}
```

## Frontend UX

Place the account UI where the current key panel lives.

States:

- Not signed in: show email, key, "使用邮箱验证码登录", "使用邮箱+密钥登录".
- Local key only: show warning "当前是本地密钥模式，数据只按本机密钥识别，建议绑定邮箱".
- Signed in: show email, active key state, logout, rotate key, send recovery code.
- Recovery: show email, code, new key input, submit.

Copy rules:

- "忘记密钥" should say "通过邮箱验证码恢复账号访问，并设置新密钥".
- Do not say "找回原密钥".
- If future true encryption is enabled, tell users old encrypted data requires recovery code or old key.

## Task 0: Link This Plan From Consolidated Handoff

**Files:**

- Create: `docs/plans/2026-05-07-v3-email-account-auth-plan.md`
- Modify: `docs/plans/2026-04-29-v3-consolidated-development-handoff-plan.md`

**Step 1: Add this plan to Source Plans**

Add:

```markdown
- `docs/plans/2026-05-07-v3-email-account-auth-plan.md`: account/email authentication plan that upgrades local-key visibility into email-bound user ownership and verification-code login.
```

**Step 2: Add account auth to security workstream**

Add a short section:

```text
Email account auth becomes the next security/product identity layer.
Local key remains as an access factor, not the only user identity.
Datasets, robots, memory, conversations, reports, static pages, and artifacts should follow user ownership.
```

**Step 3: Verify**

Run:

```powershell
git diff --check
```

Expected: pass.

## Task 1: Add Domain And Contract Types

**Files:**

- Modify: `crates/domain-model/src/lib.rs`
- Modify: `crates/contracts/src/lib.rs`
- Test: `crates/domain-model/src/lib.rs`
- Test: `crates/contracts/src/lib.rs`

**Step 1: Add auth enums**

Add:

```rust
pub enum AuthChallengePurpose {
    AccountCreate,
    Login,
    RecoverKey,
    BindEmail,
    RotateKey,
}

pub enum AuthSessionMethod {
    EmailCode,
    EmailKey,
    LocalKey,
}
```

**Step 2: Add structs**

Add:

```rust
pub struct EmailVerificationChallenge { ... }
pub struct UserSession { ... }
pub struct AuthSessionView { ... }
```

**Step 3: Add contracts**

Add:

```rust
pub struct StartEmailAuthRequest { ... }
pub struct StartEmailAuthResponse { ... }
pub struct VerifyEmailAuthRequest { ... }
pub struct VerifyEmailAuthResponse { ... }
pub struct KeyLoginRequest { ... }
pub struct KeyRotateRequest { ... }
```

**Step 4: Tests**

Add enum parse/serialize tests for snake_case values:

```rust
assert_eq!(AuthChallengePurpose::from_str("recover_key"), Some(AuthChallengePurpose::RecoverKey));
```

**Step 5: Verify**

Run:

```powershell
wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo test -p domain-model auth"
wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo test -p contracts auth"
```

Expected: pass.

## Task 2: Add Storage Migration And Repositories

**Files:**

- Create: `crates/storage/migrations/0004_email_account_auth.sql`
- Modify: `crates/storage/src/lib.rs`
- Test: `crates/storage/src/lib.rs`

**Step 1: Ensure migration runner handles ordered migrations**

If `PgStorage::migrate()` still only applies the initial schema, fix ordered migrations first. Do not add a dead migration.

**Step 2: Create auth tables**

Create:

```sql
alter table users
    add column if not exists email_normalized text,
    add column if not exists email_verified_at timestamptz,
    add column if not exists auth_state text not null default 'active',
    add column if not exists primary_secret_fingerprint text,
    add column if not exists last_login_at timestamptz,
    add column if not exists metadata jsonb not null default '{}'::jsonb;

create unique index if not exists users_tenant_email_normalized_idx
    on users (tenant_id, email_normalized)
    where email_normalized is not null;

create table if not exists user_sessions (...);
create table if not exists email_verification_challenges (...);
```

**Step 3: Add owner columns**

Add nullable first:

```sql
alter table datasets add column if not exists owner_user_id uuid references users (id) on delete set null;
alter table documents add column if not exists owner_user_id uuid references users (id) on delete set null;
alter table assistant_runs add column if not exists user_id uuid references users (id) on delete set null;
alter table conversation_memory_items add column if not exists user_id uuid references users (id) on delete set null;
alter table report_plans add column if not exists owner_user_id uuid references users (id) on delete set null;
alter table static_page_drafts add column if not exists owner_user_id uuid references users (id) on delete set null;
alter table static_page_render_outputs add column if not exists owner_user_id uuid references users (id) on delete set null;
```

**Step 4: Add repositories**

Add:

```rust
PgUserRepository::ensure_by_email(...)
PgUserSessionRepository::create/lookup/revoke(...)
PgEmailVerificationRepository::create/consume/increment_attempt(...)
```

**Step 5: Verify**

Run:

```powershell
wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo test -p storage auth"
```

Expected: pass.

## Task 3: Add OTP Service And Email Sender Trait

**Files:**

- Modify: `crates/platform-api/src/lib.rs`
- Create if useful: `crates/platform-api/src/auth_email.rs`
- Test: `crates/platform-api/src/auth_email.rs`

**Step 1: Add `EmailSender` trait**

Add:

```rust
pub trait EmailSender {
    async fn send_verification_code(&self, to: &str, purpose: AuthChallengePurpose, code: &str) -> Result<()>;
}
```

**Step 2: Add implementations**

First:

- `LoggingEmailSender` for local development.
- `CloudflareEmailSender` for production.

**Step 3: Generate OTP safely**

Rules:

- 6 digits or 8 alphanumeric characters.
- hash with server pepper before storing.
- expire in 10 minutes.
- max 5 attempts.
- throttle per email and IP/device.

**Step 4: Tests**

Add tests:

```rust
otp_hash_does_not_store_plain_code
expired_code_is_rejected
wrong_code_increments_attempts
consumed_code_cannot_be_reused
```

**Step 5: Verify**

Run:

```powershell
wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo test -p platform-api auth_email"
```

Expected: pass.

## Task 4: Add Auth API Routes

**Files:**

- Modify: `crates/platform-api/src/lib.rs`
- Modify: `crates/contracts/src/lib.rs`
- Test: `crates/platform-api/src/lib.rs`

**Step 1: Add routes**

Add:

```text
POST /v1/auth/email/start
POST /v1/auth/email/verify
POST /v1/auth/key/login
POST /v1/auth/logout
GET  /v1/auth/session
POST /v1/auth/key/rotate
POST /v1/auth/email/bind
```

**Step 2: Session cookie**

Use httpOnly secure cookie in production:

```text
aidp_v3_session=<opaque-token>
```

Store only token hash in DB.

**Step 3: Preserve local development**

If no session exists, continue supporting current local-dev tenant and public datasets. Do not break ordinary chat.

**Step 4: Tests**

Add:

```rust
email_start_creates_challenge_and_sends_email
email_verify_creates_session
key_login_resolves_user_and_session
logout_revokes_session
session_endpoint_returns_current_user
```

**Step 5: Verify**

Run:

```powershell
wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo test -p platform-api auth"
```

Expected: pass.

## Task 5: Attach User Ownership To Existing Flows

**Files:**

- Modify: `crates/platform-api/src/lib.rs`
- Modify: `crates/storage/src/lib.rs`
- Test: `crates/platform-api/src/lib.rs`

**Step 1: Dataset create/list**

Rules:

- Public datasets visible to all.
- Private datasets visible only if owner user matches or valid grant exists.
- New datasets get `owner_user_id` when session exists.

**Step 2: Upload/register document**

Rules:

- Uploaded documents inherit current user.
- Uploaded documents inherit current active secret binding or account key factor.

**Step 3: AssistantRun and memory**

Rules:

- AssistantRun stores `user_id` when session exists.
- Hidden conversation memory is partitioned by `user_id + local_thread_id`.

**Step 4: Reports/static pages**

Rules:

- Drafts and artifacts belong to user.
- Right shelf filters by user plus public/shared artifacts.

**Step 5: Tests**

Add:

```rust
private_dataset_is_hidden_from_different_user
public_dataset_is_visible_without_session
assistant_run_records_session_user
static_page_shelf_filters_by_user
```

**Step 6: Verify**

Run:

```powershell
wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo test -p platform-api dataset_visibility"
wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo test -p platform-api assistant_run"
```

Expected: pass.

## Task 6: Account UI In Existing Key Panel

**Files:**

- Modify: `apps/web/app/HomePageClient.js`
- Modify: `apps/web/app/components/Sidebar.js`
- Create: `apps/web/app/lib/account-auth.js`
- Test: `apps/web/app/lib/account-auth.test.mjs`

**Step 1: Add local account auth helpers**

Add functions:

```javascript
normalizeEmail(value)
validateEmail(value)
summarizeAccountState(state)
buildStartEmailAuthPayload(email, purpose)
buildVerifyEmailAuthPayload(email, code, purpose, deviceFingerprint)
```

**Step 2: Update key panel**

Add:

- email input.
- local key input.
- send code button.
- code input.
- login / bind / recover actions.
- current account email display.

**Step 3: Keep UI compact**

Do not add a separate login page yet.

**Step 4: Tests**

Run:

```powershell
node --test apps/web/app/lib/account-auth.test.mjs
```

Expected: pass.

**Step 5: Build**

Run:

```powershell
npm run build
```

Expected: pass.

Note: run the build from `apps/web` because the repository root currently has no npm workspace definition.

## Task 7: Cloudflare Email Sender Setup

**Files:**

- Create: `docs/operations/cloudflare-email-auth-setup.md`
- Optional later: `infra/cloudflare/email-auth-worker/wrangler.jsonc`
- Optional later: `infra/cloudflare/email-auth-worker/src/index.ts`

**Step 1: Document Cloudflare setup**

Document:

- enable Cloudflare Email Service for the domain;
- create dedicated sender such as `verify@<domain>`;
- configure DNS/domain verification;
- create API token or Worker binding;
- configure V3 env vars.

**Step 2: Choose first integration mode**

Preferred for V3 simplicity:

```text
platform-api -> Cloudflare Email Service REST API
```

Alternative for isolation:

```text
platform-api -> Cloudflare Worker -> Email Service binding
```

**Step 3: Document rate limits and failure behavior**

Rules:

- sending failure returns "验证码发送失败，请稍后再试";
- challenge may be created only if email send succeeds, or marked `send_failed`;
- repeated sends are throttled.

**Step 4: Verify docs**

Run:

```powershell
git diff --check
```

Expected: pass.

## Task 8: Migration And Backward Compatibility

**Files:**

- Modify tests/docs as needed.

**Step 1: Existing local-only users**

Behavior:

- Existing local-key-only workflow still works.
- Data without `owner_user_id` remains visible according to current public/private/key rules.
- Binding an email claims future user ownership; old records can be claimed only through local key proof.

**Step 2: Public datasets**

Behavior:

- Public datasets remain visible to unsigned users.
- Creating private datasets without account should warn that access is tied to the current local key/device.

**Step 3: Account claim flow**

Add later:

```text
POST /v1/auth/claim-local-data
```

Requires:

- logged-in user;
- current local key fingerprint;
- matching visible secret bindings.

**Step 4: Verify**

Run:

```powershell
wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo fmt --all --check"
wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo test -p storage auth"
wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo test -p platform-api auth"
wsl bash -lc "cd /mnt/c/Users/soulzyn/Desktop/codex/ai-data-platform-v3 && cargo test -p platform-api dataset_visibility"
node --test apps/web/app/lib/account-auth.test.mjs
Push-Location apps/web; npm run build; Pop-Location
```

Expected: pass.

## Acceptance Criteria

- Users can bind an email in the existing key panel.
- Users can log in with email + key.
- Users can log in with email verification code.
- Users can recover account access by email code and set a new local key.
- The product never claims to recover the old raw key.
- Datasets, robots, conversations, memory, reports, static pages, and artifacts can be scoped by `user_id`.
- Public datasets remain available without login.
- Private data from other users is invisible and unusable.
- Cloudflare email credentials are server-side only.
- OTP challenge storage is hashed, expiring, single-use, and rate-limited.

## Recommended Next Thread Prompt

```text
Continue AI Data Platform V3 from docs/plans/2026-05-07-v3-email-account-auth-plan.md.
Implement account/email auth as an upgrade to the existing local-key model, not a replacement that breaks current public/local usage.
Start with Task 0 and Task 1: link the plan, then add domain and contract types.
Use Cloudflare Email Service only as the verification-code delivery channel; V3 owns users, sessions, OTP challenges, data ownership, grants, and audit.
Never email or store raw local keys.
```
