// The phone button in the chat starts or ends a voice call. The island's
// VoiceHost (island/voice.ts) fills this in; views only call it.

export const CallControl = {
  toggle: () => {},
  /** Closes or reopens the microphone; the call stays up. */
  mute: () => {},
  /** Stops him mid-answer and listens. */
  interrupt: () => {},
  /** One spoken message into the text field, to edit before sending. */
  dictate: () => {},
};

/** The microphone level for the call's visualiser, 0…1, without re-rendering the island. */
export const MicLevel = {
  value: 0,
  listeners: new Set<(level: number) => void>(),
  set(level: number) {
    this.value = level;
    for (const l of this.listeners) l(level);
  },
};
