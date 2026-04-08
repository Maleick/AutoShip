import { useState, useEffect, type FormEvent } from "react";
import { X, Eye, EyeSlash, Key } from "@phosphor-icons/react";
import type { Account, AccountStatus, CreateAccountPayload, UpdateAccountPayload } from "../types";

const EQ_SERVERS = [
  "Firiona Vie",
  "Rizlona",
  "Mischief",
  "Thornblade",
  "Vaniki",
  "Oakwynd",
  "Teek",
  "Tormax",
  "Test Server",
];

const EQ_CLASSES = [
  "WAR", "CLR", "PAL", "RNG", "SHD", "DRU", "MNK", "BRD",
  "ROG", "SHM", "NEC", "WIZ", "MAG", "ENC", "BST", "BER", "UNK",
];

interface Props {
  account: Account | null; // null = create mode
  onClose: () => void;
  onSave: (payload: CreateAccountPayload | UpdateAccountPayload, isNew: boolean) => Promise<void>;
}

export default function AccountModal({ account, onClose, onSave }: Props) {
  const isNew = account === null;

  const [name, setName] = useState(account?.name ?? "");
  const [server, setServer] = useState(account?.server ?? EQ_SERVERS[0]);
  const [character, setCharacter] = useState(account?.character ?? "");
  const [cls, setCls] = useState(account?.class ?? "WAR");
  const [group, setGroup] = useState(account?.group ?? 0);
  const [status, setStatus] = useState<AccountStatus>(account?.status ?? "active");
  const [password, setPassword] = useState("");
  const [showPassword, setShowPassword] = useState(false);
  const [saving, setSaving] = useState(false);
  const [fieldError, setFieldError] = useState<string | null>(null);

  useEffect(() => {
    if (account) {
      setName(account.name);
      setServer(account.server);
      setCharacter(account.character);
      setCls(account.class);
      setGroup(account.group);
      setStatus(account.status);
    }
  }, [account]);

  const handleSubmit = async (e: FormEvent<HTMLFormElement>) => {
    e.preventDefault();
    setFieldError(null);

    if (!name.trim()) {
      setFieldError("Account name is required");
      return;
    }
    if (!character.trim()) {
      setFieldError("Character name is required");
      return;
    }

    setSaving(true);
    try {
      if (isNew) {
        const payload: CreateAccountPayload = {
          name: name.trim(),
          server,
          character: character.trim(),
          class: cls,
          group,
          status,
          ...(password ? { password } : {}),
        };
        await onSave(payload, true);
      } else {
        const payload: UpdateAccountPayload = {
          server,
          character: character.trim(),
          class: cls,
          group,
          status,
          ...(password ? { password } : {}),
        };
        await onSave(payload, false);
      }
      onClose();
    } catch (e) {
      setFieldError(e instanceof Error ? e.message : "Save failed");
    } finally {
      setSaving(false);
    }
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/70 backdrop-blur-sm">
      <div className="bg-void border border-magentadark/50 w-full max-w-lg shadow-[0_0_40px_rgba(204,68,255,0.15)] relative">
        {/* Header */}
        <div className="flex items-center justify-between px-6 py-4 border-b border-white/10 bg-violet/40">
          <h2 className="font-archaic text-lg text-white tracking-wider">
            {isNew ? "Enlist New Account" : `Edit Account: ${account?.name}`}
          </h2>
          <button
            onClick={onClose}
            className="text-white/40 hover:text-white transition-colors"
          >
            <X size={20} />
          </button>
        </div>

        {/* Form */}
        <form onSubmit={handleSubmit} className="p-6 flex flex-col gap-4">
          {/* Account name (read-only when editing) */}
          <div>
            <label className="block text-xs text-white/50 uppercase tracking-widest mb-1 font-rune">
              Account Name
            </label>
            <input
              value={name}
              onChange={(e) => setName(e.target.value)}
              disabled={!isNew}
              placeholder="e.g. eq_account_01"
              className="w-full bg-void border border-white/20 text-white text-sm px-3 py-2 focus:outline-none focus:border-magentaglow font-rune placeholder:text-white/20 disabled:opacity-50 disabled:cursor-not-allowed"
            />
          </div>

          {/* Server + Class */}
          <div className="grid grid-cols-2 gap-3">
            <div>
              <label className="block text-xs text-white/50 uppercase tracking-widest mb-1 font-rune">
                Server
              </label>
              <select
                value={server}
                onChange={(e) => setServer(e.target.value)}
                className="w-full bg-void border border-white/20 text-white text-sm px-3 py-2 focus:outline-none focus:border-magentaglow font-rune"
              >
                {EQ_SERVERS.map((s) => (
                  <option key={s} value={s}>{s}</option>
                ))}
              </select>
            </div>
            <div>
              <label className="block text-xs text-white/50 uppercase tracking-widest mb-1 font-rune">
                Class
              </label>
              <select
                value={cls}
                onChange={(e) => setCls(e.target.value)}
                className="w-full bg-void border border-white/20 text-white text-sm px-3 py-2 focus:outline-none focus:border-magentaglow font-rune"
              >
                {EQ_CLASSES.map((c) => (
                  <option key={c} value={c}>{c}</option>
                ))}
              </select>
            </div>
          </div>

          {/* Character name */}
          <div>
            <label className="block text-xs text-white/50 uppercase tracking-widest mb-1 font-rune">
              Character Name
            </label>
            <input
              value={character}
              onChange={(e) => setCharacter(e.target.value)}
              placeholder="e.g. Frostreaver"
              className="w-full bg-void border border-white/20 text-white text-sm px-3 py-2 focus:outline-none focus:border-magentaglow font-rune placeholder:text-white/20"
            />
          </div>

          {/* Group + Status */}
          <div className="grid grid-cols-2 gap-3">
            <div>
              <label className="block text-xs text-white/50 uppercase tracking-widest mb-1 font-rune">
                Group ID
              </label>
              <input
                type="number"
                min={0}
                value={group}
                onChange={(e) => setGroup(Number(e.target.value))}
                className="w-full bg-void border border-white/20 text-white text-sm px-3 py-2 focus:outline-none focus:border-magentaglow font-rune"
              />
            </div>
            <div>
              <label className="block text-xs text-white/50 uppercase tracking-widest mb-1 font-rune">
                Status
              </label>
              <select
                value={status}
                onChange={(e) => setStatus(e.target.value as AccountStatus)}
                className="w-full bg-void border border-white/20 text-white text-sm px-3 py-2 focus:outline-none focus:border-magentaglow font-rune"
              >
                <option value="active">Active</option>
                <option value="locked">Locked</option>
                <option value="banned">Banned</option>
              </select>
            </div>
          </div>

          {/* Password */}
          <div>
            <label className="block text-xs text-white/50 uppercase tracking-widest mb-1 font-rune flex items-center gap-1">
              <Key size={12} />
              {isNew ? "Password (optional)" : "New Password (leave blank to keep)"}
            </label>
            <div className="relative">
              <input
                type={showPassword ? "text" : "password"}
                value={password}
                onChange={(e) => setPassword(e.target.value)}
                placeholder="Stored via AES-256-GCM"
                autoComplete="new-password"
                className="w-full bg-void border border-white/20 text-white text-sm px-3 py-2 pr-10 focus:outline-none focus:border-spectral font-rune placeholder:text-white/20"
              />
              <button
                type="button"
                onClick={() => setShowPassword((v) => !v)}
                className="absolute right-3 top-1/2 -translate-y-1/2 text-white/40 hover:text-white transition-colors"
              >
                {showPassword ? <EyeSlash size={16} /> : <Eye size={16} />}
              </button>
            </div>
            {!isNew && account?.has_password && (
              <p className="text-[10px] text-spectral/70 mt-1 font-rune">
                ✓ Password on file — leave blank to keep existing
              </p>
            )}
          </div>

          {/* Error */}
          {fieldError && (
            <p className="text-red-400 text-xs font-rune bg-red-900/20 border border-red-900/40 px-3 py-2">
              {fieldError}
            </p>
          )}

          {/* Actions */}
          <div className="flex gap-3 pt-2">
            <button
              type="button"
              onClick={onClose}
              className="flex-1 px-4 py-2 border border-white/20 text-white/60 text-sm hover:bg-white/5 transition-colors font-tech uppercase tracking-wider"
            >
              Cancel
            </button>
            <button
              type="submit"
              disabled={saving}
              className="flex-1 px-4 py-2 bg-magentadark/30 border border-magentaglow text-white text-sm hover:bg-magentadark/50 transition-colors font-tech uppercase tracking-wider shadow-[0_0_15px_rgba(204,68,255,0.2)] disabled:opacity-50"
            >
              {saving ? "Saving…" : isNew ? "Enlist" : "Update"}
            </button>
          </div>
        </form>
      </div>
    </div>
  );
}
