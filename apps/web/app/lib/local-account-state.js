import { normalizeAccountEmail } from './account-auth.js';

export const LOCAL_SECRET_BINDING_IDS_STORAGE_KEY = 'aidp-v3-secret-binding-ids';
export const LOCAL_SECRET_VALUE_STORAGE_KEY = 'aidp-v3-local-secret-value';
export const LOCAL_ACCOUNT_EMAIL_STORAGE_KEY = 'aidp-v3-account-email';

export function readLocalSecretBindingIdsHeader() {
  if (typeof window === 'undefined') {
    return '';
  }
  try {
    const raw = window.localStorage.getItem(LOCAL_SECRET_BINDING_IDS_STORAGE_KEY) || '';
    return raw
      .split(',')
      .map((item) => item.trim())
      .filter(Boolean)
      .join(',');
  } catch {
    return '';
  }
}

export function readLocalSecretBindingIds() {
  return readLocalSecretBindingIdsHeader()
    .split(',')
    .map((item) => item.trim())
    .filter(Boolean);
}

export function writeLocalSecretState(secretValue, secretBindingIds) {
  if (typeof window === 'undefined') {
    return;
  }
  const normalizedBindingIds = [...new Set((secretBindingIds || []).filter(Boolean))];
  window.localStorage.setItem(LOCAL_SECRET_BINDING_IDS_STORAGE_KEY, normalizedBindingIds.join(','));
  if (secretValue) {
    window.localStorage.setItem(LOCAL_SECRET_VALUE_STORAGE_KEY, secretValue);
  }
}

export function clearLocalSecretState() {
  if (typeof window === 'undefined') {
    return;
  }
  window.localStorage.removeItem(LOCAL_SECRET_BINDING_IDS_STORAGE_KEY);
  window.localStorage.removeItem(LOCAL_SECRET_VALUE_STORAGE_KEY);
}

export function readLocalSecretValue() {
  if (typeof window === 'undefined') {
    return '';
  }
  try {
    return window.localStorage.getItem(LOCAL_SECRET_VALUE_STORAGE_KEY) || '';
  } catch {
    return '';
  }
}

export function readLocalAccountEmail() {
  if (typeof window === 'undefined') {
    return '';
  }
  try {
    return window.localStorage.getItem(LOCAL_ACCOUNT_EMAIL_STORAGE_KEY) || '';
  } catch {
    return '';
  }
}

export function writeLocalAccountEmail(email) {
  if (typeof window === 'undefined') {
    return;
  }
  try {
    const normalized = normalizeAccountEmail(email);
    if (normalized) {
      window.localStorage.setItem(LOCAL_ACCOUNT_EMAIL_STORAGE_KEY, normalized);
    } else {
      window.localStorage.removeItem(LOCAL_ACCOUNT_EMAIL_STORAGE_KEY);
    }
  } catch {
    // Account email is a convenience cache; auth is still cookie based.
  }
}
