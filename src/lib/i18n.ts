/**
 * i18n — Phase 9.1.
 *
 * A tiny, dependency-light translation layer, mirroring SundayStage: a
 * per-language catalog, a `t()` that falls back to English for any missing
 * key, and a persisted locale store. English + Norwegian are complete; the
 * other five locales (matching the Sunday suite: sv, da, de, fr, pl) ship the
 * navigation/chrome strings and fall back to English for the rest — the full
 * translation is mechanical follow-up.
 *
 * The machinery is what matters: every user-visible string goes through `t()`,
 * and adding a language is just another catalog entry. `{name}`-style tokens
 * are interpolated by `translate`.
 */

import { create } from "zustand";

export type Lang = "no" | "en" | "sv" | "da" | "de" | "fr" | "pl";

export const LANGS: Lang[] = ["no", "en", "sv", "da", "de", "fr", "pl"];

/** Autonyms — a language is shown in its own name, the same in every locale. */
export const LANG_NAMES: Record<Lang, string> = {
  no: "Norsk",
  en: "English",
  sv: "Svenska",
  da: "Dansk",
  de: "Deutsch",
  fr: "Français",
  pl: "Polski",
};

type Catalog = Record<string, string>;

// English is the source of truth: it defines every key and is the fallback for
// any string a partial locale hasn't translated yet.
const en = {
  // ── App shell / navigation ──────────────────────────────────────────────
  navTranscribe: "Transcribe",
  navEditor: "Editor",
  navTimeline: "Timeline",
  navContext: "Context & glossary",
  navSpeakers: "Speakers",
  navPolish: "AI punctuation",
  navSuggest: "Smart suggestions",
  navClips: "AI clips",
  navTranslate: "Translate",
  navCleanup: "Cleanup",
  navStyle: "Style",
  navExport: "Export",
  navSettings: "Settings",
  navBackToImport: "Back to import",
  importDemoLink: "…or explore the demo project (no video)",
  updateFailed: "Update failed: {error}",
  updateInstalling: "Downloading and installing…",
  updateAvailable: "Version {version} is available.",
  updateNow: "Update now",
  updateLater: "Later",

  // ── Common actions / shared bits ────────────────────────────────────────
  actionSave: "Save",
  actionCancel: "Cancel",
  actionAdd: "Add",
  actionClose: "Close",
  errorPrefix: "Error: {error}",
  doneFile: "Done: {path}",
  estCaptionsCost: "{n} captions · ~{cost}",
  apiKeyPlaceholder:
    "Anthropic API key (optional — otherwise ANTHROPIC_API_KEY)",
  modelHaikuHint: "Fast and cheap — recommended",
  modelSonnetHint: "Higher quality",
  modelOpusHint: "Maximum quality",

  // ── Settings ────────────────────────────────────────────────────────────
  settingsApiKeys: "API keys",
  settingsIntro:
    "Keys are stored in the operating system's keychain (Keychain on Mac, Credential Manager on Windows) — never in plaintext and never in project files. SundayEdit only shows whether a key is set, not its value.",
  settingsKeySaved: "Key saved to the keychain.",
  settingsKeyRemoved: "Key removed.",
  settingsSet: "set",
  settingsKeyStored: "•••••••• (stored)",
  settingsKeyPlaceholder: "Paste key",
  settingsRemoveKey: "Remove key",
  settingsLanguage: "Language",
  settingsLanguageIntro: "Choose the language for SundayEdit's interface.",
  providerAnthropicNote:
    "Used by AI punctuation, smart suggestions and translation.",
  providerCloudNote: "Cloud transcription.",

  // ── Model picker ────────────────────────────────────────────────────────
  modelTitle: "Choose transcription model",
  modelIntro:
    "Runs entirely on your machine — nothing is uploaded. Larger models are more accurate but slower.",
  modelRecommended: "Recommended",
  modelDownloaded: "Downloaded",
  modelDownload: "Download",
  modelDownloading: "Downloading…",
  modelCancelDownload: "Cancel download",

  // ── Local transcription ─────────────────────────────────────────────────
  localTitle: "Local transcription",
  localPrivacyBadge: "private · on your machine",
  localIntro:
    "Whisper runs locally on your machine. The video never leaves the machine, it costs nothing per minute, and it works offline. The project's context and glossary are used to steer recognition.",
  localExtracting: "Extracting audio…",
  localLoadingModel: "Loading model…",
  localTranscribingPct: "Transcribing… {pct}%",
  localTranscribe: "Transcribe locally",
  localUsingModelHint:
    "Uses the model selected above. The time depends on the video length and your machine.",
  localFailed: "Local transcription failed: {error}",
  localNeedModel:
    "Pick and download a Whisper model above to transcribe locally.",

  // ── Cloud transcription ─────────────────────────────────────────────────
  cloudTitle: "Cloud transcription (optional)",
  cloudIntro:
    "Local Whisper is the default and keeps the video on your machine. The cloud sometimes gives higher accuracy, but the audio is uploaded and costs per minute — so it's off by default and requires explicit consent.",
  cloudSegmentOnly: "segment confidence only",
  cloudSegmentOnlyTitle:
    "Killer feature #1 (confidence) is only estimated at segment level",
  cloudKeySet: "key set",
  cloudKeyMissing: "key missing (Settings)",
  cloudEstimatedLabel: "Estimated:",
  cloudForMinutes: "for {minutes} min",
  cloudPrivacy: "Privacy",
  cloudSelected: "Selected",
  cloudSelect: "Select",
  cloudTranscribing: "Transcribing in the cloud…",
  cloudTranscribeWith: "Transcribe with {provider}",
  cloudUploadHint:
    "Uploads the project's audio to {provider}. Short clips work best (API limit ~25 MB).",
  cloudAssemblyNote:
    " AssemblyAI uploads and waits for a result, so it may take a while.",
  cloudNeedKey:
    "Add your {provider} key under Settings → API keys to transcribe in the cloud.",
  cloudTranscribeFailed: "Transcription failed: {error}",
  consentTitle: "Upload audio to {provider}?",
  consentReadPrivacy: "Read the privacy policy",
  consentAccept: "I understand — continue",

  // ── Cleanup ─────────────────────────────────────────────────────────────
  cleanupFindReplace: "Search and replace",
  cleanupSearchPlaceholder: "Search…",
  cleanupFind: "Find",
  cleanupReplacePlaceholder: "Replace with… (empty = delete)",
  cleanupReplaceAll: "Replace all",
  cleanupCaseTitle: "Match case",
  cleanupWholeWordLabel: "Word",
  cleanupWholeWordTitle: "Whole words only",
  cleanupRegexTitle: "Regular expression",
  cleanupMatches: "{n} matches",
  cleanupNoMatches: "No matches to replace.",
  cleanupFillerTitle: "Remove fillers",
  cleanupFillerIntro:
    "Find «eh», «uh», «um», «like» etc. Approved clips are removed and the rest of the timeline shifts earlier (ripple).",
  cleanupFindFillers: "Find fillers",
  cleanupNoFillers: "No fillers found 🎉",
  cleanupRemoveSelected: "Remove {n} selected",
  cleanupFound: "{n} found",

  // ── Context & glossary ──────────────────────────────────────────────────
  contextIntro:
    "Tell SundayEdit what the recording is about and which names/terms appear. That makes recognition more accurate (priming) and auto-corrects known misspellings.",
  contextDescriptionLabel: "Description",
  contextDescriptionPlaceholder:
    "E.g. «A sermon on christology and soteriology. The speaker is Norwegian.»",
  contextGlossaryLabel: "Glossary ({n})",
  contextSuggestTerms: "Suggest terms (AI)",
  contextSuggestTitle: "Let AI suggest terms from the transcript",
  contextAddTerm: "Add term",
  contextNoNewTerms: "Found no new terms to suggest.",
  contextSuggestError: "Error: {error} (add an Anthropic key in Settings?)",
  contextSuggestionsHeader: "{n} suggestions — accept the ones you want",
  contextDismiss: "Dismiss suggestion",
  contextNoTermsYet:
    "No terms yet. Add names, jargon or foreign words Whisper should expect.",
  contextFieldTerm: "Term (correct form)",
  contextFieldAliases: "Misspellings (comma)",
  contextFieldDefinition: "Definition (optional)",
  contextFieldPronunciation: "Pronunciation (optional)",
  contextRemoveTerm: "Remove term",
  contextApplyNow: "Correct terms on the captions now",
  contextNoTermsToCorrect: "No terms to correct.",
  contextCorrected: "Corrected {n} occurrence(s).",

  // ── AI punctuation (polish) ─────────────────────────────────────────────
  polishTitle: "AI punctuation",
  polishIntro:
    "Only corrects punctuation and capitalization. The words are never changed — attempts to change content are rejected automatically and listed below.",
  polishRun: "Polish punctuation",
  polishRunning: "Polishing…",
  polishRejected:
    "{n} caption(s) were rejected because the model tried to change the words themselves. They were kept unchanged.",
  polishNoChanges:
    "No changes needed — the punctuation already looked good. 🎉",
  polishChangesHeader: "{n} change(s)",

  // ── AI clips ────────────────────────────────────────────────────────────
  clipsTitle: "AI clips for social media",
  clipsIntro:
    "Find the short, self-contained moments in the talk — each with a clear title and hook. The timings come from the actual captions, so the model can never invent timing. Nothing is saved until you press «Apply plan».",
  clipsGenerate: "Suggest clips",
  clipsRegenerate: "Generate again",
  clipsFinding: "Finding clips…",
  clipsNeedCaptions:
    "Transcribe the talk first — the clips are built from the captions.",
  clipsSummaryLabel: "Talk summary",
  clipsSummaryPlaceholder: "Short summary of the whole talk…",
  clipsCountHeader: "{n} clips",
  clipsNoneInPlan: "No clips in the plan. Generate again or adjust the talk.",
  clipsApply: "Apply plan",
  clipsApplied: "Saved to the project",
  clipsTitlePlaceholder: "Title (shown as overlay)",
  clipsHookPlaceholder: "Hook (one line)",
  clipsCaptionsCount: "{n} captions",
  clipsRemove: "Remove clip",
  clipsRenderVertical: "Render vertically",
  clipsRendering: "Burning in…",
} satisfies Catalog;

const no: Catalog = {
  // ── App shell / navigation ──────────────────────────────────────────────
  navTranscribe: "Transkriber",
  navEditor: "Editor",
  navTimeline: "Tidslinje",
  navContext: "Kontekst og ordliste",
  navSpeakers: "Talere",
  navPolish: "AI-tegnsetting",
  navSuggest: "Smarte forslag",
  navClips: "AI-klipp",
  navTranslate: "Oversett",
  navCleanup: "Opprydding",
  navStyle: "Stil",
  navExport: "Eksport",
  navSettings: "Innstillinger",
  navBackToImport: "Tilbake til import",
  importDemoLink: "…eller utforsk demo-prosjektet (uten video)",
  updateFailed: "Oppdatering feilet: {error}",
  updateInstalling: "Laster ned og installerer…",
  updateAvailable: "Ny versjon {version} er tilgjengelig.",
  updateNow: "Oppdater nå",
  updateLater: "Senere",

  // ── Common actions / shared bits ────────────────────────────────────────
  actionSave: "Lagre",
  actionCancel: "Avbryt",
  actionAdd: "Legg til",
  actionClose: "Lukk",
  errorPrefix: "Feil: {error}",
  doneFile: "Ferdig: {path}",
  estCaptionsCost: "{n} undertekster · ~{cost}",
  apiKeyPlaceholder:
    "Anthropic API-nøkkel (valgfritt — ellers ANTHROPIC_API_KEY)",
  modelHaikuHint: "Rask og billig — anbefalt",
  modelSonnetHint: "Høyere kvalitet",
  modelOpusHint: "Maks kvalitet",

  // ── Settings ────────────────────────────────────────────────────────────
  settingsApiKeys: "API-nøkler",
  settingsIntro:
    "Nøkler lagres i operativsystemets nøkkelring (Keychain på Mac, Credential Manager på Windows) — aldri i klartekst og aldri i prosjektfiler. SundayEdit viser kun om en nøkkel er satt, ikke selve verdien.",
  settingsKeySaved: "Nøkkel lagret i nøkkelringen.",
  settingsKeyRemoved: "Nøkkel fjernet.",
  settingsSet: "satt",
  settingsKeyStored: "•••••••• (lagret)",
  settingsKeyPlaceholder: "Lim inn nøkkel",
  settingsRemoveKey: "Fjern nøkkel",
  settingsLanguage: "Språk",
  settingsLanguageIntro: "Velg språk for SundayEdit-grensesnittet.",
  providerAnthropicNote:
    "Brukes av AI-tegnsetting, smarte forslag og oversettelse.",
  providerCloudNote: "Sky-transkripsjon.",

  // ── Model picker ────────────────────────────────────────────────────────
  modelTitle: "Velg transkripsjonsmodell",
  modelIntro:
    "Kjører helt lokalt på maskinen din — ingenting lastes opp. Større modeller er mer nøyaktige men tregere.",
  modelRecommended: "Anbefalt",
  modelDownloaded: "Lastet ned",
  modelDownload: "Last ned",
  modelDownloading: "Laster ned…",
  modelCancelDownload: "Avbryt nedlasting",

  // ── Local transcription ─────────────────────────────────────────────────
  localTitle: "Lokal transkripsjon",
  localPrivacyBadge: "personvern · på maskinen",
  localIntro:
    "Whisper kjører lokalt på maskinen din. Videoen forlater aldri maskinen, det koster ingenting per minutt, og det fungerer uten nett. Kontekst og ordliste fra prosjektet brukes til å styre gjenkjenningen.",
  localExtracting: "Henter ut lyd…",
  localLoadingModel: "Laster modell…",
  localTranscribingPct: "Transkriberer… {pct}%",
  localTranscribe: "Transkriber lokalt",
  localUsingModelHint:
    "Bruker modellen valgt over. Lengden avhenger av videovarighet og maskinen din.",
  localFailed: "Lokal transkripsjon feilet: {error}",
  localNeedModel:
    "Velg og last ned en Whisper-modell over for å transkribere lokalt.",

  // ── Cloud transcription ─────────────────────────────────────────────────
  cloudTitle: "Sky-transkripsjon (valgfritt)",
  cloudIntro:
    "Lokal Whisper er standard og holder videoen på maskinen din. Sky gir av og til høyere nøyaktighet, men lyden lastes opp og koster per minutt — derfor er den av som standard og krever et bevisst samtykke.",
  cloudSegmentOnly: "kun segment-confidence",
  cloudSegmentOnlyTitle:
    "Killer-feature #1 (confidence) blir bare anslått fra segment-nivå",
  cloudKeySet: "nøkkel satt",
  cloudKeyMissing: "nøkkel mangler (Innstillinger)",
  cloudEstimatedLabel: "Estimert:",
  cloudForMinutes: "for {minutes} min",
  cloudPrivacy: "Personvern",
  cloudSelected: "Valgt",
  cloudSelect: "Velg",
  cloudTranscribing: "Transkriberer i skyen…",
  cloudTranscribeWith: "Transkriber med {provider}",
  cloudUploadHint:
    "Laster opp prosjektets lyd til {provider}. Korte klipp fungerer best (API-grense ~25 MB).",
  cloudAssemblyNote:
    " AssemblyAI laster opp og venter på resultat, så det kan ta litt tid.",
  cloudNeedKey:
    "Legg inn {provider}-nøkkelen din under Innstillinger → API-nøkler for å transkribere i skyen.",
  cloudTranscribeFailed: "Transkripsjon feilet: {error}",
  consentTitle: "Last opp lyd til {provider}?",
  consentReadPrivacy: "Les personvernerklæringen",
  consentAccept: "Jeg forstår — fortsett",

  // ── Cleanup ─────────────────────────────────────────────────────────────
  cleanupFindReplace: "Søk og erstatt",
  cleanupSearchPlaceholder: "Søk…",
  cleanupFind: "Finn",
  cleanupReplacePlaceholder: "Erstatt med… (tom = slett)",
  cleanupReplaceAll: "Erstatt alle",
  cleanupCaseTitle: "Skill store/små",
  cleanupWholeWordLabel: "Ord",
  cleanupWholeWordTitle: "Kun hele ord",
  cleanupRegexTitle: "Regulært uttrykk",
  cleanupMatches: "{n} treff",
  cleanupNoMatches: "Ingen treff å erstatte.",
  cleanupFillerTitle: "Fjern fyllord",
  cleanupFillerIntro:
    "Finn «eh», «øh», «um», «liksom» osv. Godkjente klipp fjernes og resten av tidslinjen forskyves tidligere (ripple).",
  cleanupFindFillers: "Finn fyllord",
  cleanupNoFillers: "Ingen fyllord funnet 🎉",
  cleanupRemoveSelected: "Fjern {n} valgte",
  cleanupFound: "{n} funnet",

  // ── Context & glossary ──────────────────────────────────────────────────
  contextIntro:
    "Fortell SundayEdit hva opptaket handler om og hvilke navn/fagord som forekommer. Det gjør gjenkjenningen mer nøyaktig (priming) og retter kjente feilstavinger automatisk.",
  contextDescriptionLabel: "Beskrivelse",
  contextDescriptionPlaceholder:
    "F.eks. «En preken om kristologi og soteriologi. Taleren er norsk.»",
  contextGlossaryLabel: "Ordliste ({n})",
  contextSuggestTerms: "Foreslå termer (AI)",
  contextSuggestTitle: "La AI foreslå termer fra transkripsjonen",
  contextAddTerm: "Legg til term",
  contextNoNewTerms: "Fant ingen nye termer å foreslå.",
  contextSuggestError:
    "Feil: {error} (legg inn Anthropic-nøkkel i Innstillinger?)",
  contextSuggestionsHeader: "{n} forslag — godta dem du vil ha",
  contextDismiss: "Forkast forslag",
  contextNoTermsYet:
    "Ingen termer ennå. Legg til navn, fagord eller fremmedord som Whisper bør forvente.",
  contextFieldTerm: "Term (riktig form)",
  contextFieldAliases: "Feilstavinger (komma)",
  contextFieldDefinition: "Definisjon (valgfritt)",
  contextFieldPronunciation: "Uttale (valgfritt)",
  contextRemoveTerm: "Fjern term",
  contextApplyNow: "Rett termer på undertekstene nå",
  contextNoTermsToCorrect: "Ingen termer å rette.",
  contextCorrected: "Rettet {n} forekomst(er).",

  // ── AI punctuation (polish) ─────────────────────────────────────────────
  polishTitle: "AI-tegnsetting",
  polishIntro:
    "Retter kun tegnsetting og store/små bokstaver. Ordene endres aldri — forsøk på å endre innhold avvises automatisk og listes nedenfor.",
  polishRun: "Poler tegnsetting",
  polishRunning: "Polerer…",
  polishRejected:
    "{n} undertekst(er) ble avvist fordi modellen prøvde å endre selve ordene. De ble beholdt uendret.",
  polishNoChanges:
    "Ingen endringer trengtes — tegnsettingen så allerede bra ut. 🎉",
  polishChangesHeader: "{n} endring(er)",

  // ── AI clips ────────────────────────────────────────────────────────────
  clipsTitle: "AI-klipp for sosiale medier",
  clipsIntro:
    "Finn de korte, selvstendige øyeblikkene i talen — hver med en tydelig tittel og hook. Tidspunktene hentes fra de faktiske undertekstene, så modellen kan aldri finne på timing. Ingenting lagres før du trykker «Bruk plan».",
  clipsGenerate: "Foreslå klipp",
  clipsRegenerate: "Generer på nytt",
  clipsFinding: "Finner klipp…",
  clipsNeedCaptions:
    "Transkriber talen først — klippene bygges på undertekstene.",
  clipsSummaryLabel: "Sammendrag av talen",
  clipsSummaryPlaceholder: "Kort sammendrag av hele talen…",
  clipsCountHeader: "{n} klipp",
  clipsNoneInPlan: "Ingen klipp i planen. Generer på nytt eller juster talen.",
  clipsApply: "Bruk plan",
  clipsApplied: "Lagret på prosjektet",
  clipsTitlePlaceholder: "Tittel (vises som overlay)",
  clipsHookPlaceholder: "Hook (én linje)",
  clipsCaptionsCount: "{n} undertekster",
  clipsRemove: "Fjern klipp",
  clipsRenderVertical: "Render vertikalt",
  clipsRendering: "Brenner inn…",
};

// Scandinavian + de/fr/pl: nav/chrome only; everything else falls back to en.
const sv: Catalog = {
  navTranscribe: "Transkribera",
  navEditor: "Editor",
  navTimeline: "Tidslinje",
  navContext: "Kontext & ordlista",
  navSpeakers: "Talare",
  navPolish: "AI-interpunktion",
  navSuggest: "Smarta förslag",
  navClips: "AI-klipp",
  navTranslate: "Översätt",
  navCleanup: "Städning",
  navStyle: "Stil",
  navExport: "Export",
  navSettings: "Inställningar",
  navBackToImport: "Tillbaka till import",
};
const da: Catalog = {
  navTranscribe: "Transskriber",
  navEditor: "Editor",
  navTimeline: "Tidslinje",
  navContext: "Kontekst & ordliste",
  navSpeakers: "Talere",
  navPolish: "AI-tegnsætning",
  navSuggest: "Smarte forslag",
  navClips: "AI-klip",
  navTranslate: "Oversæt",
  navCleanup: "Oprydning",
  navStyle: "Stil",
  navExport: "Eksport",
  navSettings: "Indstillinger",
  navBackToImport: "Tilbage til import",
};
const de: Catalog = {
  navTranscribe: "Transkribieren",
  navEditor: "Editor",
  navTimeline: "Zeitleiste",
  navContext: "Kontext & Glossar",
  navSpeakers: "Sprecher",
  navPolish: "KI-Zeichensetzung",
  navSuggest: "Intelligente Vorschläge",
  navClips: "KI-Clips",
  navTranslate: "Übersetzen",
  navCleanup: "Bereinigen",
  navStyle: "Stil",
  navExport: "Export",
  navSettings: "Einstellungen",
  navBackToImport: "Zurück zum Import",
};
const fr: Catalog = {
  navTranscribe: "Transcrire",
  navEditor: "Éditeur",
  navTimeline: "Chronologie",
  navContext: "Contexte et glossaire",
  navSpeakers: "Intervenants",
  navPolish: "Ponctuation IA",
  navSuggest: "Suggestions intelligentes",
  navClips: "Clips IA",
  navTranslate: "Traduire",
  navCleanup: "Nettoyage",
  navStyle: "Style",
  navExport: "Export",
  navSettings: "Paramètres",
  navBackToImport: "Retour à l'import",
};
const pl: Catalog = {
  navTranscribe: "Transkrybuj",
  navEditor: "Edytor",
  navTimeline: "Oś czasu",
  navContext: "Kontekst i słownik",
  navSpeakers: "Mówcy",
  navPolish: "Interpunkcja AI",
  navSuggest: "Inteligentne sugestie",
  navClips: "Klipy AI",
  navTranslate: "Tłumacz",
  navCleanup: "Czyszczenie",
  navStyle: "Styl",
  navExport: "Eksport",
  navSettings: "Ustawienia",
  navBackToImport: "Powrót do importu",
};

const CATALOG: Record<Lang, Catalog> = { en, no, sv, da, de, fr, pl };

export type TKey = keyof typeof en;

/** Optional `{name}`-style interpolation values. */
export type TParams = Record<string, string | number>;

export function translate(lang: Lang, key: TKey, params?: TParams): string {
  let s = CATALOG[lang]?.[key] ?? en[key] ?? key;
  if (params) {
    for (const [k, v] of Object.entries(params)) {
      s = s.split(`{${k}}`).join(String(v));
    }
  }
  return s;
}

// ── Persisted locale store ─────────────────────────────────────────────────────

const STORAGE_KEY = "sundayedit.locale";

function initialLang(): Lang {
  try {
    const saved = localStorage.getItem(STORAGE_KEY) as Lang | null;
    if (saved && LANGS.includes(saved)) return saved;
  } catch {
    /* localStorage may be unavailable */
  }
  return "no";
}

interface LocaleState {
  lang: Lang;
  setLang: (lang: Lang) => void;
}

export const useLocale = create<LocaleState>((set) => ({
  lang: initialLang(),
  setLang: (lang) => {
    try {
      localStorage.setItem(STORAGE_KEY, lang);
    } catch {
      /* ignore */
    }
    set({ lang });
  },
}));

/** Hook returning a `t` bound to the current locale. */
export function useT(): (key: TKey, params?: TParams) => string {
  const lang = useLocale((s) => s.lang);
  return (key, params) => translate(lang, key, params);
}
