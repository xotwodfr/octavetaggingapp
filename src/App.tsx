import { useEffect, useMemo, useState } from "react";
import { api } from "./api";
import type { AppSettings, MusicBrainzResult, ScanResult, TrackDraft, TrackRecord, TrackTags } from "./types";

const DEFAULT_SETTINGS: AppSettings = {
  spicyLyricsApiKey: "",
  sourceFolder: "",
  destinationFolder: "",
  filenameTemplate: "{track:02} - {title}",
  folderTemplate: "{artist}/{album}",
  embedLyrics: true,
  saveLyricsSidecar: false,
};

function sanitizeComponent(s: string): string {
  const cleaned = s.replace(/[<>:"/\\|?*\x00-\x1f]/g, "_").trim().replace(/\.+$/g, "");
  return cleaned || "_";
}

function renderTemplate(template: string, tags: TrackTags, fallback: string): string {
  const title = tags.title || fallback;
  const artist = tags.artist || "Unknown Artist";
  const album = tags.album || "Unknown Album";
  const albumArtist = tags.albumArtist || artist;
  const date = tags.date || "";
  const track = tags.trackNumber ?? 0;
  const trackTotal = tags.trackTotal ?? 0;
  const disc = tags.discNumber ?? 0;
  const pairs: [string, string][] = [
    ["{title}", sanitizeComponent(title)],
    ["{artist}", sanitizeComponent(artist)],
    ["{album}", sanitizeComponent(album)],
    ["{albumartist}", sanitizeComponent(albumArtist)],
    ["{date}", sanitizeComponent(date)],
    ["{track:02}", String(track).padStart(2, "0")],
    ["{track}", String(track)],
    ["{tracktotal}", String(trackTotal)],
    ["{disc}", String(disc)],
  ];
  let out = template;
  for (const [k, v] of pairs) out = out.split(k).join(v);
  return out;
}

function buildTargetPath(track: TrackRecord, draft: TrackDraft, settings: AppSettings): string {
  const base = settings.destinationFolder?.trim() || track.path.replace(/[\\/][^\\/]+$/, "");
  const folder = renderTemplate(settings.folderTemplate, draft.tags, track.parsed.normalizedBase);
  const file = renderTemplate(settings.filenameTemplate, draft.tags, track.parsed.normalizedBase);
  const sep = base.includes("\\") ? "\\" : "/";
  const folderPath = folder.split("/").filter(Boolean).map(sanitizeComponent).join(sep);
  return [base.replace(/[\\/]+$/, ""), folderPath, `${sanitizeComponent(file)}.flac`].filter(Boolean).join(sep);
}

function initialDraft(track: TrackRecord, settings: AppSettings): TrackDraft {
  const tags: TrackTags = {
    ...track.tags,
    title: track.tags.title || track.parsed.title || track.fileName.replace(/\.[^.]+$/, ""),
    artist: track.tags.artist || track.parsed.artist || null,
  };
  const draft: TrackDraft = {
    selected: track.isFlac,
    tags,
    coverArtUrl: null,
    lyricsStatus: "idle",
    lyricsMessage: null,
    targetPath: "",
  };
  draft.targetPath = buildTargetPath(track, draft, settings);
  return draft;
}

function formatBytes(n: number): string {
  if (!n) return "0 B";
  const units = ["B", "KB", "MB", "GB"];
  let i = 0;
  let v = n;
  while (v >= 1024 && i < units.length - 1) { v /= 1024; i++; }
  return `${v.toFixed(i === 0 ? 0 : 1)} ${units[i]}`;
}

export default function App() {
  const [settings, setSettings] = useState<AppSettings>(DEFAULT_SETTINGS);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [scan, setScan] = useState<ScanResult | null>(null);
  const [drafts, setDrafts] = useState<Record<string, TrackDraft>>({});
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const [log, setLog] = useState<string[]>([]);
  const [mbQuery, setMbQuery] = useState({ artist: "", title: "", album: "" });
  const [mbResults, setMbResults] = useState<MusicBrainzResult[]>([]);

  useEffect(() => {
    api.loadSettings().then((s) => setSettings({ ...DEFAULT_SETTINGS, ...s })).catch((e) => addLog(`Settings load failed: ${e}`));
  }, []);

  useEffect(() => {
    if (!scan) return;
    setDrafts((prev) => {
      const next: Record<string, TrackDraft> = {};
      for (const t of scan.tracks) {
        const existing = prev[t.id];
        next[t.id] = existing ? { ...existing, targetPath: buildTargetPath(t, existing, settings) } : initialDraft(t, settings);
      }
      return next;
    });
  }, [settings.filenameTemplate, settings.folderTemplate, settings.destinationFolder, scan]);

  const tracks = scan?.tracks ?? [];
  const selectedTrack = tracks.find((t) => t.id === selectedId) ?? tracks[0] ?? null;
  const selectedDraft = selectedTrack ? drafts[selectedTrack.id] : null;
  const selectedCount = useMemo(() => Object.values(drafts).filter((d) => d.selected).length, [drafts]);

  function addLog(msg: string) {
    setLog((l) => [`${new Date().toLocaleTimeString()} ${msg}`, ...l].slice(0, 100));
  }

  function updateDraft(id: string, patch: Partial<TrackDraft> | ((d: TrackDraft) => Partial<TrackDraft>)) {
    setDrafts((prev) => {
      const cur = prev[id];
      if (!cur) return prev;
      const p = typeof patch === "function" ? patch(cur) : patch;
      const next = { ...cur, ...p };
      const track = tracks.find((t) => t.id === id);
      if (track) next.targetPath = buildTargetPath(track, next, settings);
      return { ...prev, [id]: next };
    });
  }

  function updateTag(id: string, key: keyof TrackTags, value: string) {
    updateDraft(id, (d) => {
      const tags = { ...d.tags };
      if (["trackNumber", "trackTotal", "discNumber", "discTotal"].includes(key)) {
        const n = value.trim() === "" ? null : Number(value);
        (tags as any)[key] = Number.isFinite(n) ? n : null;
      } else if (key === "hasCoverArt") {
        tags.hasCoverArt = value === "true";
      } else {
        (tags as any)[key] = value || null;
      }
      return { tags };
    });
  }

  async function chooseFolder(kind: "source" | "destination") {
    try {
      const folder = await api.pickFolder();
      if (!folder) return;
      setSettings((s) => ({ ...s, [kind === "source" ? "sourceFolder" : "destinationFolder"]: folder }));
    } catch (e) { addLog(`Folder pick failed: ${e}`); }
  }

  async function runScan() {
    const folder = settings.sourceFolder?.trim();
    if (!folder) return addLog("Select a source folder first.");
    setBusy("Scanning...");
    try {
      const res = await api.scanFolder(folder);
      setScan(res);
      const nextDrafts: Record<string, TrackDraft> = {};
      for (const t of res.tracks) nextDrafts[t.id] = initialDraft(t, settings);
      setDrafts(nextDrafts);
      setSelectedId(res.tracks[0]?.id ?? null);
      addLog(`Scanned ${res.scannedCount} files (${res.flacCount} FLAC, ${res.lossyCount} lossy).`);
    } catch (e) { addLog(`Scan failed: ${e}`); }
    finally { setBusy(null); }
  }

  async function saveSettings() {
    try {
      const saved = await api.saveSettings(settings);
      setSettings({ ...DEFAULT_SETTINGS, ...saved });
      setSettingsOpen(false);
      addLog("Settings saved.");
    } catch (e) { addLog(`Save settings failed: ${e}`); }
  }

  async function searchMb() {
    setBusy("Searching MusicBrainz...");
    try {
      const res = await api.lookupMusicBrainz({
        artist: mbQuery.artist || selectedDraft?.tags.artist || selectedTrack?.parsed.artist || undefined,
        title: mbQuery.title || selectedDraft?.tags.title || selectedTrack?.parsed.title || undefined,
        album: mbQuery.album || selectedDraft?.tags.album || undefined,
        limit: 12,
      });
      setMbResults(res);
      addLog(`MusicBrainz returned ${res.length} results.`);
    } catch (e) { addLog(`MusicBrainz search failed: ${e}`); }
    finally { setBusy(null); }
  }

  function applyMbResult(r: MusicBrainzResult, batch: boolean) {
    const ids = batch ? Object.entries(drafts).filter(([, d]) => d.selected).map(([id]) => id) : (selectedTrack ? [selectedTrack.id] : []);
    for (const id of ids) {
      updateDraft(id, (d) => ({
        tags: {
          ...d.tags,
          title: batch ? d.tags.title : (r.title || d.tags.title),
          artist: r.artist || d.tags.artist,
          album: r.album || d.tags.album,
          albumArtist: r.albumArtist || r.artist || d.tags.albumArtist,
          date: r.date || d.tags.date,
          trackNumber: batch ? d.tags.trackNumber : (r.trackNumber ?? d.tags.trackNumber),
          trackTotal: r.trackTotal ?? d.tags.trackTotal,
        },
        coverArtUrl: r.coverArtUrl || d.coverArtUrl,
      }));
    }
    addLog(`${batch ? "Batch-applied" : "Applied"} MusicBrainz release ${r.album || r.title}.`);
  }

  async function lookupLyrics(id: string) {
    const track = tracks.find((t) => t.id === id);
    const draft = drafts[id];
    if (!track || !draft) return;
    const artist = draft.tags.artist || "";
    const title = draft.tags.title || "";
    updateDraft(id, { lyricsStatus: "loading", lyricsMessage: null });
    try {
      const res = await api.lookupSpicyLyrics({
        apiKey: settings.spicyLyricsApiKey,
        artist,
        title,
        album: draft.tags.album || undefined,
      });
      if (res.status === "found" && res.lyrics) {
        updateDraft(id, (d) => ({ tags: { ...d.tags, lyrics: res.lyrics || null }, lyricsStatus: "found", lyricsMessage: res.format ? `Found (${res.format})` : "Found" }));
      } else {
        updateDraft(id, { lyricsStatus: res.status === "not_found" ? "not_found" : "error", lyricsMessage: res.message || res.status });
      }
    } catch (e) {
      updateDraft(id, { lyricsStatus: "error", lyricsMessage: String(e) });
    }
  }

  async function lookupLyricsBatch() {
    const ids = Object.entries(drafts).filter(([, d]) => d.selected).map(([id]) => id);
    setBusy(`Lyrics 0/${ids.length}`);
    let i = 0;
    for (const id of ids) {
      i++;
      setBusy(`Lyrics ${i}/${ids.length}`);
      await lookupLyrics(id);
    }
    setBusy(null);
    addLog(`Lyrics lookup finished for ${ids.length} tracks.`);
  }

  async function applyChanges() {
    const changes = Object.entries(drafts)
      .filter(([, d]) => d.selected)
      .map(([id, d]) => ({
        id,
        sourcePath: id,
        targetPath: d.targetPath,
        tags: d.tags,
        coverArtUrl: d.coverArtUrl || null,
        lyrics: d.tags.lyrics || null,
        embedLyrics: settings.embedLyrics,
        saveLyricsSidecar: settings.saveLyricsSidecar,
      }));
    if (!changes.length) return addLog("No tracks selected.");
    setBusy(`Applying 0/${changes.length}`);
    try {
      const results = await api.applyChanges({ changes, filenameTemplate: settings.filenameTemplate, folderTemplate: settings.folderTemplate });
      setDrafts((prev) => {
        const next = { ...prev };
        for (const r of results) {
          if (!next[r.id]) continue;
          next[r.id] = { ...next[r.id], applyStatus: r.ok ? "ok" : "error", applyMessage: r.message, lyricsStatus: (r.lyricsStatus as any) || next[r.id].lyricsStatus };
        }
        return next;
      });
      addLog(`Apply finished: ${results.filter((r) => r.ok).length}/${results.length} succeeded.`);
      if (settings.sourceFolder) await runScan();
    } catch (e) { addLog(`Apply failed: ${e}`); }
    finally { setBusy(null); }
  }

  return (
    <div className="min-h-screen bg-zinc-950 text-zinc-100">
      <header className="border-b border-zinc-800 px-5 py-4 flex items-center gap-3">
        <div>
          <h1 className="text-xl font-semibold">Octave Tagging</h1>
          <p className="text-xs text-zinc-400">Verify, tag, organize, and enrich lossless exports for Navidrome</p>
        </div>
        <div className="ml-auto flex items-center gap-2">
          {busy && <span className="text-xs text-amber-300">{busy}</span>}
          <button className="px-3 py-1.5 rounded bg-zinc-800 hover:bg-zinc-700 text-sm" onClick={() => setSettingsOpen((v) => !v)}>Settings</button>
        </div>
      </header>

      <div className="p-4 grid grid-cols-12 gap-4">
        <section className="col-span-12 xl:col-span-4 bg-zinc-900/60 border border-zinc-800 rounded-lg p-3 space-y-3">
          <div className="flex gap-2">
            <input className="flex-1 bg-zinc-950 border border-zinc-800 rounded px-2 py-1.5 text-sm" placeholder="Source folder" value={settings.sourceFolder || ""} onChange={(e) => setSettings({ ...settings, sourceFolder: e.target.value })} />
            <button className="px-2 py-1.5 bg-zinc-800 rounded text-sm" onClick={() => chooseFolder("source")}>Pick</button>
            <button className="px-2 py-1.5 bg-emerald-700 hover:bg-emerald-600 rounded text-sm" onClick={runScan}>Scan</button>
          </div>
          <div className="flex gap-2">
            <input className="flex-1 bg-zinc-950 border border-zinc-800 rounded px-2 py-1.5 text-sm" placeholder="Destination/library folder (optional)" value={settings.destinationFolder || ""} onChange={(e) => setSettings({ ...settings, destinationFolder: e.target.value })} />
            <button className="px-2 py-1.5 bg-zinc-800 rounded text-sm" onClick={() => chooseFolder("destination")}>Pick</button>
          </div>
          <div className="text-xs text-zinc-400 flex gap-3">
            <span>{scan?.scannedCount ?? 0} files</span><span>{scan?.flacCount ?? 0} FLAC</span><span>{scan?.lossyCount ?? 0} lossy</span><span>{selectedCount} selected</span>
          </div>
          <div className="max-h-[60vh] overflow-auto border border-zinc-800 rounded">
            {tracks.map((t) => {
              const d = drafts[t.id];
              return (
                <div key={t.id} className={`px-2 py-2 border-b border-zinc-800 cursor-pointer ${selectedTrack?.id === t.id ? "bg-zinc-800/80" : "hover:bg-zinc-800/40"}`} onClick={() => setSelectedId(t.id)}>
                  <div className="flex items-center gap-2">
                    <input type="checkbox" checked={!!d?.selected} onChange={(e) => updateDraft(t.id, { selected: e.target.checked })} onClick={(e) => e.stopPropagation()} />
                    <div className="min-w-0 flex-1">
                      <div className="text-sm truncate">{d?.tags.title || t.parsed.title || t.fileName}</div>
                      <div className="text-xs text-zinc-400 truncate">{d?.tags.artist || t.parsed.artist || "Unknown artist"} · {t.extension.toUpperCase()}</div>
                    </div>
                    <span className={`text-[10px] px-1.5 py-0.5 rounded ${t.isLossy ? "bg-red-900/60 text-red-200" : t.verification?.ok ? "bg-emerald-900/60 text-emerald-200" : "bg-amber-900/60 text-amber-200"}`}>{t.isLossy ? "LOSSY" : t.verification?.ok ? "OK" : "CHECK"}</span>
                  </div>
                  {d?.applyMessage && <div className={`text-[11px] mt-1 ${d.applyStatus === "ok" ? "text-emerald-300" : "text-red-300"}`}>{d.applyMessage}</div>}
                </div>
              );
            })}
            {!tracks.length && <div className="p-4 text-sm text-zinc-500">Pick a source folder and scan.</div>}
          </div>
        </section>

        <section className="col-span-12 xl:col-span-5 bg-zinc-900/60 border border-zinc-800 rounded-lg p-3 space-y-3">
          <h2 className="font-medium">Track Details & Tags</h2>
          {selectedTrack && selectedDraft ? (
            <>
              <div className="text-xs text-zinc-400 break-all">{selectedTrack.path}</div>
              <div className="grid grid-cols-2 gap-2 text-xs text-zinc-300">
                <div>Codec: {selectedTrack.stream?.codecName || "?"}</div>
                <div>Rate: {selectedTrack.stream?.sampleRate ? `${selectedTrack.stream.sampleRate} Hz` : "?"}</div>
                <div>Bits: {selectedTrack.stream?.bitsPerRawSample ?? "?"}</div>
                <div>Channels: {selectedTrack.stream?.channels ?? "?"}</div>
                <div>Size: {formatBytes(selectedTrack.sizeBytes)}</div>
                <div>Verify: {selectedTrack.verification?.ok ? "OK" : "Issue"}</div>
              </div>
              {selectedTrack.verification && <div className={`text-xs p-2 rounded ${selectedTrack.verification.ok ? "bg-emerald-950/50 text-emerald-200" : "bg-amber-950/50 text-amber-200"}`}>{selectedTrack.verification.tool}: {selectedTrack.verification.message}</div>}
              {selectedTrack.warnings.length > 0 && <div className="text-xs text-amber-300">{selectedTrack.warnings.join(" ")}</div>}
              <div className="grid grid-cols-2 gap-2">
                {([
                  ["title", "Title"], ["artist", "Artist"], ["album", "Album"], ["albumArtist", "Album Artist"],
                  ["date", "Date"], ["trackNumber", "Track #"], ["trackTotal", "Track Total"], ["discNumber", "Disc #"],
                ] as [keyof TrackTags, string][]).map(([k, label]) => (
                  <label key={k} className="text-xs space-y-1">
                    <div className="text-zinc-400">{label}</div>
                    <input className="w-full bg-zinc-950 border border-zinc-800 rounded px-2 py-1.5 text-sm" value={(selectedDraft.tags[k] as any) ?? ""} onChange={(e) => updateTag(selectedTrack.id, k, e.target.value)} />
                  </label>
                ))}
              </div>
              <div className="flex gap-2">
                <button className="px-2 py-1.5 bg-zinc-800 rounded text-sm" onClick={() => lookupLyrics(selectedTrack.id)}>Lookup Lyrics</button>
                <button className="px-2 py-1.5 bg-zinc-800 rounded text-sm" onClick={lookupLyricsBatch}>Batch Lyrics</button>
                <span className="text-xs text-zinc-400 self-center">Lyrics: {selectedDraft.lyricsStatus}{selectedDraft.lyricsMessage ? ` - ${selectedDraft.lyricsMessage}` : ""}</span>
              </div>
              <textarea className="w-full h-28 bg-zinc-950 border border-zinc-800 rounded px-2 py-1.5 text-xs font-mono" placeholder="Lyrics / TTML" value={selectedDraft.tags.lyrics || ""} onChange={(e) => updateTag(selectedTrack.id, "lyrics", e.target.value)} />
            </>
          ) : <div className="text-sm text-zinc-500">No track selected.</div>}
        </section>

        <section className="col-span-12 xl:col-span-3 bg-zinc-900/60 border border-zinc-800 rounded-lg p-3 space-y-3">
          <h2 className="font-medium">MusicBrainz & Apply</h2>
          <div className="space-y-2">
            {(["artist", "title", "album"] as const).map((k) => (
              <input key={k} className="w-full bg-zinc-950 border border-zinc-800 rounded px-2 py-1.5 text-sm" placeholder={`Search ${k}`} value={mbQuery[k]} onChange={(e) => setMbQuery({ ...mbQuery, [k]: e.target.value })} />
            ))}
            <button className="w-full px-2 py-1.5 bg-blue-700 hover:bg-blue-600 rounded text-sm" onClick={searchMb}>Search MusicBrainz</button>
          </div>
          <div className="max-h-52 overflow-auto space-y-2">
            {mbResults.map((r, i) => (
              <div key={`${r.recordingId}-${r.releaseId}-${i}`} className="border border-zinc-800 rounded p-2 text-xs">
                <div className="font-medium">{r.title}</div>
                <div className="text-zinc-400">{r.artist} — {r.album}</div>
                <div className="text-zinc-500">{r.date || ""} {r.trackNumber ? `#${r.trackNumber}` : ""}</div>
                <div className="flex gap-2 mt-2">
                  <button className="px-2 py-1 bg-zinc-800 rounded" onClick={() => applyMbResult(r, false)}>Apply</button>
                  <button className="px-2 py-1 bg-zinc-800 rounded" onClick={() => applyMbResult(r, true)}>Batch</button>
                </div>
              </div>
            ))}
          </div>
          <div className="border-t border-zinc-800 pt-3 space-y-2">
            <div className="text-xs text-zinc-400">Preview target (selected track)</div>
            <div className="text-xs bg-zinc-950 border border-zinc-800 rounded p-2 break-all min-h-10">{selectedDraft?.targetPath || "-"}</div>
            <button className="w-full px-3 py-2 bg-emerald-700 hover:bg-emerald-600 rounded font-medium" onClick={applyChanges}>Apply Selected</button>
          </div>
          <div className="border-t border-zinc-800 pt-3">
            <div className="text-xs text-zinc-400 mb-1">Activity</div>
            <div className="max-h-40 overflow-auto text-[11px] text-zinc-400 space-y-1">
              {log.map((l, i) => <div key={i}>{l}</div>)}
            </div>
          </div>
        </section>
      </div>

      {settingsOpen && (
        <div className="fixed inset-0 bg-black/60 flex items-center justify-center p-4">
          <div className="w-full max-w-2xl bg-zinc-900 border border-zinc-700 rounded-lg p-4 space-y-3">
            <h3 className="text-lg font-medium">Settings</h3>
            <label className="text-sm space-y-1 block">
              <div className="text-zinc-400">Spicy Lyrics API key</div>
              <input type="password" className="w-full bg-zinc-950 border border-zinc-800 rounded px-2 py-1.5" value={settings.spicyLyricsApiKey} onChange={(e) => setSettings({ ...settings, spicyLyricsApiKey: e.target.value })} />
            </label>
            <div className="grid grid-cols-2 gap-2">
              <label className="text-sm space-y-1 block">
                <div className="text-zinc-400">Filename template</div>
                <input className="w-full bg-zinc-950 border border-zinc-800 rounded px-2 py-1.5" value={settings.filenameTemplate} onChange={(e) => setSettings({ ...settings, filenameTemplate: e.target.value })} />
              </label>
              <label className="text-sm space-y-1 block">
                <div className="text-zinc-400">Folder template</div>
                <input className="w-full bg-zinc-950 border border-zinc-800 rounded px-2 py-1.5" value={settings.folderTemplate} onChange={(e) => setSettings({ ...settings, folderTemplate: e.target.value })} />
              </label>
            </div>
            <div className="flex gap-4 text-sm">
              <label className="flex items-center gap-2"><input type="checkbox" checked={settings.embedLyrics} onChange={(e) => setSettings({ ...settings, embedLyrics: e.target.checked })} /> Embed lyrics in FLAC tags</label>
              <label className="flex items-center gap-2"><input type="checkbox" checked={settings.saveLyricsSidecar} onChange={(e) => setSettings({ ...settings, saveLyricsSidecar: e.target.checked })} /> Save .ttml sidecar</label>
            </div>
            <div className="text-xs text-zinc-500">Templates: {"{artist} {album} {albumartist} {title} {date} {track:02} {track} {tracktotal} {disc}"}</div>
            <div className="flex justify-end gap-2">
              <button className="px-3 py-1.5 bg-zinc-800 rounded" onClick={() => setSettingsOpen(false)}>Cancel</button>
              <button className="px-3 py-1.5 bg-emerald-700 rounded" onClick={saveSettings}>Save</button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
