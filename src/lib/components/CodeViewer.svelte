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

    $: mergeView && update(value, newValue);

    // There may be a more graceful way to update the two editors
    function update(a: string, b: string): void {
        mergeView?.destroy()
        mergeView = create_merge_view(a, b)
    }

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
