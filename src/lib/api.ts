import { invoke } from "@tauri-apps/api/core";

export type OsProfile = "mac" | "windows";

export type GeoMode = "auto" | "manual";

export type ProxyStatus =
  | { kind: "unknown" }
  | { kind: "ok"; ip: string; country: string; latency_ms: number }
  | { kind: "dead" };

export type RunStatus =
  | { kind: "stopped" }
  | { kind: "running"; pid: number; cdp_port: number };

export interface Profile {
  id: string;
  name: string;
  seed: number;
  os_profile: OsProfile;
  proxy: string | null;
  proxy_status: ProxyStatus;
  tags: string[];
  group: string | null;
  notes: string;
  language_mode: GeoMode;
  language: string | null;
  timezone_mode: GeoMode;
  timezone: string | null;
  status: RunStatus;
  created_at: string;
  updated_at: string;
}

export interface NewProfile {
  name: string;
  os_profile: OsProfile;
  proxy: string | null;
  tags: string[];
  group: string | null;
  notes: string;
  language_mode: GeoMode;
  language: string | null;
  timezone_mode: GeoMode;
  timezone: string | null;
}

export const api = {
  list: () => invoke<Profile[]>("list_profiles"),
  create: (newp: NewProfile) => invoke<Profile>("create_profile", { new: newp }),
  update: (profile: Profile) => invoke<void>("update_profile", { profile }),
  remove: (id: string) => invoke<void>("delete_profile", { id }),
  setEnginePath: (path: string) => invoke<void>("set_engine_path", { path }),
  engineConfigured: () => invoke<boolean>("engine_configured"),
  launch: (id: string) => invoke<number>("launch_profile", { id }),
  stop: (id: string) => invoke<void>("stop_profile", { id }),
  checkProxy: (id: string) => invoke<void>("check_proxy", { id }),
  // Returns per-source error messages (empty array = all installed OK).
  addExtensions: (id: string, sources: string[]) => invoke<string[]>("add_extensions", { id, sources }),
  getDefaultExtensions: () => invoke<string[]>("get_default_extensions"),
  setDefaultExtensions: (sources: string[]) => invoke<void>("set_default_extensions", { sources }),
  clone: (id: string, name: string, inheritProxy: boolean) =>
    invoke<Profile>("clone_profile_cmd", { id, name, inheritProxy }),
  checkForUpdate: (force: boolean) =>
    invoke<{ version: string } | null>("check_for_update", { force }),
  downloadUpdate: (version: string) => invoke<void>("download_update", { version }),
  downloadEngine: () => invoke<string>("download_engine"),
  profileSize: (id: string) => invoke<number>("profile_size", { id }),
  clearProfileCache: (id: string) => invoke<number>("clear_profile_cache", { id }),
  clearAllCaches: () => invoke<number>("clear_all_caches"),
};
