import { Doc } from "yjs";
import { diffChars } from "diff";

export type Delta =
    | { retain: number }
    | { delete: number }
    | { insert: string };

// Compute the set of Yjs delta operations (that is, `insert` and
// `delete`) required to go from initialText to finalText.
// Based on https://github.com/kpdecker/jsdiff.
const getDeltaOperations = (
    initialText: string,
    finalText: string
): Delta[] => {
    if (initialText === finalText) {
        return [];
    }

    const edits = diffChars(initialText, finalText);
    let prevOffset = 0;
    let deltas: Delta[] = [];

    for (const edit of edits) {
        if (edit.removed && edit.value) {
            deltas = [
                ...deltas,
                ...[
                    ...(prevOffset > 0 ? [{ retain: prevOffset }] : []),
                    { delete: edit.value.length },
                ],
            ];
            prevOffset = 0;
        } else if (edit.added && edit.value) {
            deltas = [...deltas, ...[{ retain: prevOffset }, { insert: edit.value }]];
            prevOffset = edit.value.length;
        } else {
            prevOffset = edit.value.length;
        }
    }
    return deltas;
};

export type HistoryEntry = { time: number; deltas: Delta[] };

export class TextDocument {
    private doc: Doc = new Doc();
    private history: HistoryEntry[] = [];

    private constructor(...history: HistoryEntry[]) {
        this.doc
            .getText()
            .applyDelta(
                history.sort((a, b) => a.time - b.time).flatMap((h) => h.deltas)
            );
        this.history = history;
    }

    static new(content?: string) {
        return new TextDocument({
            time: new Date().getTime(),
            deltas: content ? [{ insert: content }] : [],
        });
    }

    update(content: string) {
        const deltas = getDeltaOperations(this.toString(), content);
        if (deltas.length == 0) return;
        this.doc.getText().applyDelta(deltas);
        this.history.push({ time: new Date().getTime(), deltas });
    }

    getHistory() {
        return this.history.slice();
    }

    toString() {
        return this.doc.getText().toString();
    }

    at(time: number) {
        return new TextDocument(
            ...this.history.filter((entry) => entry.time <= time)
        );
    }
}
