// The voice and dictation pages of the Control Centre, by id (settings/main.ts
// draws Groovy's own pages).

import type { FlowPageRenderer, PageId } from "./shell";
import { dictationPage } from "./pages/dictation";
import { notesPage } from "./pages/notes";
import { insightsPage } from "./pages/insights";
import { dictionaryPage, snippetsPage } from "./pages/words";
import { stylesPage } from "./pages/styles";
import { flowSettingsPage } from "./pages/flowsettings";
import { voicePage } from "./pages/voice";
import { skillsPage } from "./pages/skills";

export const FLOW_PAGE_RENDERERS: Partial<Record<PageId, FlowPageRenderer>> = {
  dictation: dictationPage,
  notes: notesPage,
  insights: insightsPage,
  dictionary: dictionaryPage,
  snippets: snippetsPage,
  styles: stylesPage,
  flowsettings: flowSettingsPage,
  voice: voicePage,
  skills: skillsPage,
};
