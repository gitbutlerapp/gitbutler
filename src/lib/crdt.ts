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

export class TextDocument {
    private doc: Doc = new Doc();
    private history: { time: number; deltas: Delta[] }[] = [];

    private constructor({
        content,
        history,
    }: {
        content?: string;
        history: { time: number; deltas: Delta[] }[];
    }) {
        if (content !== undefined && history.length > 0) {
            throw new Error("only one of content and history can be set");
        } else if (content !== undefined) {
            this.doc.getText().insert(0, content);
        } else if (history.length > 0) {
            this.doc
                .getText()
                .applyDelta(
                    history.sort((a, b) => a.time - b.time).flatMap((h) => h.deltas)
                );
            this.history = history;
        }
    }

    static new(content?: string) {
        return new TextDocument({ content, history: [] });
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
        return new TextDocument({
            history: this.history.filter((entry) => entry.time <= time),
        });
    }
}
