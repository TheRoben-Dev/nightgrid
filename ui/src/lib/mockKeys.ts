/** Browser-only stand-in for the SSH key manager (M10k). */
import type { AuthorizedKey_Serialize as AuthorizedKey, KeyList_Serialize as KeyList, PushOutcome, SshKey_Serialize as SshKey } from "./bindings";

const DIR = "/home/you/.ssh";
let keys: SshKey[] = [
  {
    fingerprint: "SHA256:Qm9vdHN0cmFwLWxhcHRvcC1rZXktZXhhbXBsZQ",
    algorithm: "ssh-ed25519",
    comment: "you@laptop",
    public: "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIOMqqnkVzrm0SdG6UOoqKLsabgH5C9okWi0dh2l9GKJl you@laptop",
    file: `${DIR}/id_ed25519`,
    encrypted: false,
    inAgent: true,
  },
  {
    fingerprint: "SHA256:T2xkUlNBa2V5LWZyb20tMjAxOS1leGFtcGxlLWtleQ",
    algorithm: "ssh-rsa",
    comment: "old-rsa",
    public: "ssh-rsa AAAAB3NzaC1yc2E old-rsa",
    file: `${DIR}/id_rsa`,
    encrypted: true,
    inAgent: false,
  },
];
const STRANGER: AuthorizedKey = { line: 9, fingerprint: "SHA256:VW5rbm93bi1rZXktb24tdGhlLWhvc3Q", algorithm: "ssh-ed25519", comment: "who@where", options: null };
/** host id → fingerprints in its authorized_keys */
const onHost = new Map<number, string[]>();
const fps = (id: number) => {
  if (!onHost.has(id)) onHost.set(id, id % 3 === 0 ? [keys[1].fingerprint, STRANGER.fingerprint] : [keys[0].fingerprint]);
  return onHost.get(id) ?? [];
};
const wait = <T>(v: T, ms = 250) => new Promise<T>((r) => setTimeout(() => r(v), ms));

export const mockKeys = {
  list: (): Promise<KeyList> => wait({ dir: DIR, keys: [...keys], agentError: null }, 80),
  generate(name: string, comment: string, passphrase: string | null): Promise<SshKey> {
    if (keys.some((k) => k.file === `${DIR}/${name}`)) return Promise.reject({ message: `${DIR}/${name} already exists` });
    const k: SshKey = {
      fingerprint: `SHA256:${btoa(name + comment).replace(/=/g, "")}`,
      algorithm: "ssh-ed25519",
      comment,
      public: `ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAI${name} ${comment}`,
      file: `${DIR}/${name}`,
      encrypted: !!passphrase,
      inAgent: false,
    };
    keys = [...keys, k];
    return wait(k);
  },
  authorized: (hostId: number): Promise<AuthorizedKey[]> =>
    wait(
      fps(hostId).map((fp, i) => {
        const k = keys.find((x) => x.fingerprint === fp);
        return k ? { line: i + 1, fingerprint: fp, algorithm: k.algorithm, comment: k.comment, options: null } : { ...STRANGER, line: i + 1 };
      }),
    ),
  push(hostId: number, fp: string): Promise<PushOutcome> {
    const list = fps(hostId);
    if (list.includes(fp)) return wait("alreadyThere");
    onHost.set(hostId, [...list, fp]);
    return wait("added");
  },
  remove(hostId: number, fp: string): Promise<number> {
    const list = fps(hostId);
    onHost.set(hostId, list.filter((x) => x !== fp));
    return wait(list.length - (onHost.get(hostId)?.length ?? 0));
  },
  verify: (): Promise<null> => wait(null, 500),
};
