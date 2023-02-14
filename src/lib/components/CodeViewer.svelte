<script lang="ts">
    import { onMount, onDestroy } from "svelte";

    import { EditorState, StateField, StateEffect } from "@codemirror/state";
    import { EditorView, lineNumbers, Decoration } from "@codemirror/view";
    import { MergeView } from "@codemirror/merge";

    export let value: string;
    export let newValue: string;

    let element: HTMLDivElement;
    let mergeView: MergeView;

    onMount(() => (mergeView = create_merge_view(value, newValue)));
    onDestroy(() => mergeView?.destroy());

    // $: mergeView && update(value, newValue);

    // function update(a: string | null | undefined, b: string | null | undefined): void {
    // mergeView.a.setState(create_editor_state(a));
    // mergeView.reconfigure({
    //     collapseUnchanged: { margin: 3, minSize: 3 },
    // })
    // view.setState(create_editor_state(value));
    // mer
    // mergeView.setA(create_editor_state(value));
    // mergeView.setB(create_editor_state(value));
    // TODO
    // }

    function create_editor_state(
        value: string | null | undefined
    ): EditorState {
        return EditorState.create({
            doc: value ?? undefined,
            extensions: state_extensions,
        });
    }

    function create_merge_view(a: string, b: string): MergeView {
        return new MergeView({
            a: create_editor_state(a),
            b: create_editor_state(b),
            parent: element,
            collapseUnchanged: { margin: 3, minSize: 3 },
        });
    }

    let state_extensions = [
        EditorView.editable.of(false),
        EditorView.lineWrapping,
        lineNumbers(),
    ];
</script>

<code>
    <div bind:this={element} />
</code>
