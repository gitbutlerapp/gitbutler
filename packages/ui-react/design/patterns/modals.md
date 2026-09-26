# Modals

**Three parts, in code and in ⚛️ Core.** `ModalHeader`, `ModalBody` and
`ModalFooter` inside `Modal`; in Figma, `Modal / Header`, `Modal / Body` and
`Modal / Footer` on the popup container. Their descriptions carry the rules
below that the code can hold; change one and change the other.

**Pick the kind before the layout.** A prompt, a confirmation or a short form
is built from the three parts. A pane that holds more than one group, like
settings or the conflict resolver, lays itself out. A picker fills itself with
`PopupSearch` and `PopupSection`. Nothing else goes in a modal.

**One question per modal.** A modal asks one thing and closes on the answer.
If it needs steps, a Back button or a choice that opens another choice, it is a
page or a panel, not a modal.

**Size by what it holds, starting at `small`.** `xsmall` (320) for a one-line
question, `small` (420) for a prompt with a field or a short list, `medium`
(820) for a pane, `large` (1100) for a working surface. Before `large`, ask
whether the content wants a page. Height follows the content; only the body
scrolls.

**`alert` only when the question has to be answered.** Credentials, and
anything that destroys work. Every other modal closes on Escape and on the
backdrop, which is why none has a close button.

**The title asks the question or names the task; the description says what
happens.** "Discard 3 files?", then "Their changes are gone for good". The
answer button repeats the verb — Discard, Upload, Continue — never "OK" or
"Yes", so the button reads right without the title. See
[Voice](../content/voice.md).

**Cancel, then the answer.** Cancel is `ghost`; the answer is `gray`, or
`danger` when it destroys something, and sits last. Never `pop`: two buttons
are not the busy surface pop is kept for. See [Buttons](../components/buttons.md)
and [Emphasis](../foundations/emphasis.md).
