# Cloudflare Email Auth Setup

This project uses Cloudflare Email Service as the server-side email delivery channel for verification codes and bounded operational exception notices. V3 still owns users, sessions, OTP challenges, ownership checks, audit records, and operational routing policy.

References checked on 2026-05-07:

- Cloudflare Email Service REST API: `https://developers.cloudflare.com/email-service/api/send-emails/rest-api/`
- Cloudflare Email Service Workers API: `https://developers.cloudflare.com/email-service/api/send-emails/workers-api/`
- Cloudflare Email Routing overview: `https://developers.cloudflare.com/email-routing/`

## Chosen Mode

Use the Email Service REST API first:

```text
platform-api -> Cloudflare Email Service REST API -> user mailbox
```

Reason:

- V3 already generates and stores the OTP challenge.
- A REST sender keeps the auth path simple and avoids adding a Worker hop before it is needed.
- Cloudflare docs list the REST endpoint as `POST https://api.cloudflare.com/client/v4/accounts/{account_id}/email/sending/send`.
- Email Routing is for inbound routing/forwarding; it is now integrated into Email Service, but new outbound sending should follow Email Service docs.

Worker binding remains a later option if we want stricter isolation:

```text
platform-api -> internal Worker endpoint -> env.EMAIL.send()
```

## Required Cloudflare Setup

1. Put the sending domain on Cloudflare DNS.
2. Enable Cloudflare Email Service for the account/domain.
3. Configure the sender address, for example `verify@yourdomain.com`.
4. Complete domain/sender authentication in Cloudflare.
5. Create an API token with permission to send emails.
6. Configure V3 server environment variables.

## V3 Environment Variables

Required for real email sending:

```text
CLOUDFLARE_EMAIL_ACCOUNT_ID=<cloudflare-account-id>
CLOUDFLARE_EMAIL_API_TOKEN=<email-sending-api-token>
CLOUDFLARE_EMAIL_SENDER=verify@yourdomain.com
```

Optional:

```text
CLOUDFLARE_EMAIL_SENDER_NAME=AI Data Platform
CLOUDFLARE_EMAIL_REPLY_TO=support@yourdomain.com
CLOUDFLARE_EMAIL_API_BASE_URL=https://api.cloudflare.com/client/v4
CODEX_HOST_FIXED_TASK_EXCEPTION_EMAIL_TO=soulzyn@qq.com
```

If the required variables are absent, V3 falls back to `LoggingEmailSender`. This keeps local development and tests usable without Cloudflare credentials.

## Security Rules

- V3 must never email the raw local key.
- V3 must never store the raw local key.
- OTP email may contain only the short verification code and purpose copy.
- Codex Host fixed-task exception email may contain only bounded audit summaries, never raw prompts, diffs, provider logs, database URLs, credentials, or full customer documents.
- Email OTP proves control of the mailbox; it does not recover the original local key.
- Cloudflare API token stays server-side only.
- Frontend talks only to V3 `/v1/auth/*`; it never calls Cloudflare directly.

## Failure Behavior

- Cloudflare send failure returns `验证码发送失败，请稍后再试`.
- V3 creates the OTP challenge only after email sending succeeds.
- Repeated sends reuse a recent challenge and expose `resend_after_seconds`.
- Cloudflare REST API failures are logged with machine-readable error code/message, but the UI receives only the safe generic error.
- Codex Host fixed-task exception email failures are logged as operational warnings; they must not block normal workflow state persistence.

## Production Checklist

- Confirm sender domain authentication is healthy.
- Confirm API token cannot manage unrelated Cloudflare resources.
- Confirm `AUTH_OTP_PEPPER` and `AUTH_SESSION_PEPPER` are set to production secrets.
- Confirm platform-api runs behind HTTPS so auth cookies can be marked secure.
- Confirm mail logs do not include OTP codes.
- Add provider-level metrics later: sent, queued, bounced, throttled, send_failed.
