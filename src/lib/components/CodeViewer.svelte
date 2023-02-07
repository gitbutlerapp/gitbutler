<script lang="ts">
    import { onMount, onDestroy } from "svelte";

    import { EditorState, StateField, StateEffect } from "@codemirror/state";
    import { EditorView, lineNumbers, Decoration } from "@codemirror/view";

    export let value: string;
    //$: value = doc?.toString();
    //$: lastInserts = doc
    //?.getHistory()
    //.filter((e) => e.deltas.some((x: any) => x["insert"]))
    //.slice(1)
    //.slice(-10)
    //.map((e) => e["deltas"]);

    let element: HTMLDivElement;
    let view: EditorView;

    $: view && update(value);
    //$: view && col(lastInserts, "bg-fuchsia-500");

    //function col(inserts: any[] | undefined, c: string) {
    //if (!inserts) return;
    //let cs = [
    //"bg-fuchsia-900",
    //"bg-fuchsia-800",
    //"bg-fuchsia-700",
    //"bg-fuchsia-600",
    //"bg-fuchsia-500",
    //"bg-fuchsia-400",
    //"bg-fuchsia-300",
    //"bg-fuchsia-200",
    //"bg-fuchsia-100",
    //"bg-fuchsia-50",
    //];
    //for (let i of inserts.reverse()) {
    //let op = cs.shift();
    //let cls = op ? op : "bg-fuchsia-50";
    //console.log(cls);
    //colorLastEdit(i, cls);
    //}
    //}

    //function colorLastEdit(e: any[] | undefined, c: string) {
    //let retain = e?.filter((e) => e["retain"])[0];
    //let insert = e?.filter((e) => e["insert"])[0];
    //if (retain && insert) {
    //let start = retain["retain"];
    //let end = start + insert["insert"].length;
    //triggerColor(view, [{ from: start, to: end, c: c }]);
    //}
    //}

    onMount(() => (view = create_editor_view()));
    onDestroy(() => view?.destroy());

    function update(value: string | null | undefined): void {
        view.setState(create_editor_state(value));
    }

    function create_editor_state(
        value: string | null | undefined
    ): EditorState {
        return EditorState.create({
            doc: value ?? undefined,
            extensions: state_extensions,
        });
    }

    function create_editor_view(): EditorView {
        return new EditorView({
            parent: element,
            state: create_editor_state(value),
        });
    }

    const addColor = StateEffect.define<{
        from: number;
        to: number;
        c: string;
    }>({
        map: ({ from, to, c }, change) => {
            return {
                from: change.mapPos(from),
                to: change.mapPos(to),
                c: c,
            };
        },
    });

    const colorField = StateField.define({
        create() {
            return Decoration.none;
        },
        update(painted, tr) {
            painted = painted.map(tr.changes);
            for (let e of tr.effects)
                if (e.is(addColor)) {
                    painted = painted.update({
                        add: [
                            Decoration.mark({ class: e.value.c }).range(
                                e.value.from,
                                e.value.to
                            ),
                        ],
                    });
                }
            return painted;
        },
        provide: (f) => EditorView.decorations.from(f),
    });

    //const triggerColor = (view: EditorView, positions: any[]) => {
    //for (const position of positions) {
    //const effect = addColor.of(position);
    //view.dispatch({
    //effects: [effect],
    //});
    //}
    //};

    let state_extensions = [
        EditorView.editable.of(false),
        EditorView.lineWrapping,
        lineNumbers(),
        colorField,
    ];
</script>

<code>
    <div bind:this={element} />
</code>
