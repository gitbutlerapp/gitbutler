import { Doc, snapshot } from "yjs";
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

export const text = (content?: string) => {
    const doc = new Doc();
    const deltas = getDeltaOperations("", content || "");
    doc.getText().applyDelta(deltas);
    let lastSnapshot = snapshot(doc);
    const snapshots = [
        { time: new Date().getTime(), deltas: doc.getText().toDelta() },
    ];
    return {
        update: (content: string) => {
            const deltas = getDeltaOperations(doc.getText().toString(), content);
            doc.getText().applyDelta(deltas);
            const newSnapshot = snapshot(doc);
            snapshots.push({
                time: new Date().getTime(),
                deltas: doc.getText().toDelta(newSnapshot, lastSnapshot),
            });
            lastSnapshot = newSnapshot;
        },
        history: () => snapshots,
        toString: () => doc.getText().toString(),
    };
};
