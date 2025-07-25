// Main UI Components
export { default as AgentAvatar } from '$components/AgentAvatar.svelte';
export { default as AsyncButton } from '$components/AsyncButton.svelte';
export { default as Badge } from '$components/Badge.svelte';
export { default as Button, type Props as ButtonProps } from '$components/Button.svelte';
export { default as Checkbox } from '$components/Checkbox.svelte';
export {
	default as CommitStatusBadge,
	type CommitStatusType
} from '$components/CommitStatusBadge.svelte';
export { default as ContextMenu } from '$components/ContextMenu.svelte';
export { default as ContextMenuItem } from '$components/ContextMenuItem.svelte';
export { default as ContextMenuSection } from '$components/ContextMenuSection.svelte';
export { default as DropDownButton } from '$components/DropDownButton.svelte';
export { default as EditorLogo } from '$components/EditorLogo.svelte';
export { default as EmptyStatePlaceholder } from '$components/EmptyStatePlaceholder.svelte';
export { default as HunkDiff, type LineClickParams } from '$components/hunkDiff/HunkDiff.svelte';
export { default as Icon, type IconName } from '$components/Icon.svelte';
export { default as InfoButton } from '$components/InfoButton.svelte';
export {
	default as IntegrationSeriesRow,
	type BranchShouldBeDeletedMap
} from '$components/IntegrationSeriesRow.svelte';
export { default as LinkButton } from '$components/LinkButton.svelte';
export { default as Modal } from '$components/Modal.svelte';
export { default as ModalHeader } from '$components/ModalHeader.svelte';
export { default as NotificationButton } from '$components/NotificationButton.svelte';
export { default as PrStatusBadge } from '$components/PrStatusBadge.svelte';
export { default as RadioButton } from '$components/RadioButton.svelte';
export { default as ReviewBadge } from '$components/ReviewBadge.svelte';
export { default as SectionCard } from '$components/SectionCard.svelte';
export { default as SeriesIcon } from '$components/SeriesIcon.svelte';
export { default as SeriesLabelsRow } from '$components/SeriesLabelsRow.svelte';
export { default as SidebarEntry } from '$components/SidebarEntry.svelte';
export { default as SimpleCommitRow } from '$components/SimpleCommitRow.svelte';
export { default as Spacer } from '$components/Spacer.svelte';
export { default as Textarea } from '$components/Textarea.svelte';
export { default as Textbox } from '$components/Textbox.svelte';
export { default as TimeAgo } from '$components/TimeAgo.svelte';
export { default as Toggle } from '$components/Toggle.svelte';
export { default as Tooltip } from '$components/Tooltip.svelte';

// Avatar Components
export { default as Avatar } from '$components/avatar/Avatar.svelte';
export { default as AvatarGroup } from '$components/avatar/AvatarGroup.svelte';

// File Components
export { default as ExecutableLabel } from '$components/file/ExecutableLabel.svelte';
export { default as FileIcon } from '$components/file/FileIcon.svelte';
export { default as FileIndent } from '$components/file/FileIndent.svelte';
export { default as FileListItem } from '$components/file/FileListItem.svelte';
export { default as FileName } from '$components/file/FileName.svelte';
export { default as FileStatusBadge } from '$components/file/FileStatusBadge.svelte';
export { default as FileViewHeader } from '$components/file/FileViewHeader.svelte';
export { default as FolderListItem } from '$components/file/FolderListItem.svelte';
export { default as LineChangeStats } from '$components/file/LineChangeStats.svelte';

// Select Components
export { default as OptionsGroup } from '$components/select/OptionsGroup.svelte';
export { default as SearchItem } from '$components/select/SearchItem.svelte';
export {
	default as Select,
	type SelectItem as SelectItemType
} from '$components/select/Select.svelte';
export { default as SelectItem } from '$components/select/SelectItem.svelte';

// Emoji Components
export { default as EmojiPickerButton } from '$components/emoji/EmojiPickerButton.svelte';
export { default as EmojiButton } from '$components/emoji/EmojiButton.svelte';
export { default as EmojiGroup } from '$components/emoji/EmojiGroup.svelte';
export { default as EmojiPicker } from '$components/emoji/EmojiPicker.svelte';

// Link Components
export { default as Link } from './components/link/Link.svelte';

// Scroll Components
export { default as ScrollableContainer } from '$components/scroll/ScrollableContainer.svelte';
export { default as Scrollbar } from '$components/scroll/Scrollbar.svelte';

// Segment Control Components
export { default as Segment } from '$components/segmentControl/Segment.svelte';
export { default as SegmentControl } from '$components/segmentControl/SegmentControl.svelte';

// Commit Lines Components
export { default as Cell } from '$components/commitLines/Cell.svelte';
export { default as CommitNode } from '$components/commitLines/CommitNode.svelte';
export { default as Line } from '$components/commitLines/Line.svelte';

// Hunk Diff Components
export { default as HunkDiffBody } from '$components/hunkDiff/HunkDiffBody.svelte';
export { default as HunkDiffRow } from '$components/hunkDiff/HunkDiffRow.svelte';

// Markdown Components
export { default as Markdown } from '$components/markdown/Markdown.svelte';
export { default as MarkdownContent } from '$components/markdown/MarkdownContent.svelte';
export { default as Blockquote } from '$components/markdown/markdownRenderers/Blockquote.svelte';
export { default as Br } from '$components/markdown/markdownRenderers/Br.svelte';
export { default as Code } from '$components/markdown/markdownRenderers/Code.svelte';
export { default as Codespan } from '$components/markdown/markdownRenderers/Codespan.svelte';
export { default as Em } from '$components/markdown/markdownRenderers/Em.svelte';
export { default as Heading } from '$components/markdown/markdownRenderers/Heading.svelte';
export { default as Html } from '$components/markdown/markdownRenderers/Html.svelte';
export { default as Image } from '$components/markdown/markdownRenderers/Image.svelte';
export { default as List } from '$components/markdown/markdownRenderers/List.svelte';
export { default as ListItem } from '$components/markdown/markdownRenderers/ListItem.svelte';
export { default as Paragraph } from '$components/markdown/markdownRenderers/Paragraph.svelte';
export { default as Strong } from '$components/markdown/markdownRenderers/Strong.svelte';
export { default as Text } from '$components/markdown/markdownRenderers/Text.svelte';

// Popover Actions Components
export { default as PopoverActionsContainer } from '$components/popoverActions/PopoverActionsContainer.svelte';
export { default as PopoverActionsItem } from '$components/popoverActions/PopoverActionsItem.svelte';

// Rich Text Components
export { default as RichTextEditor } from '$lib/richText/RichTextEditor.svelte';
export { default as Formatter } from '$lib/richText/plugins/Formatter.svelte';
export { default as GhostTextPlugin } from '$lib/richText/plugins/GhostText.svelte';
export { default as HardWrapPlugin } from '$lib/richText/plugins/HardWrapPlugin.svelte';
export {
	default as Mention,
	type MentionSuggestion,
	type MentionSuggestionUpdate
} from '$lib/richText/plugins/Mention.svelte';
export { default as FormattingBar } from '$lib/richText/tools/FormattingBar.svelte';
export { default as FormattingButton } from '$lib/richText/tools/FormattingButton.svelte';

// Utilities and other exports
export * from './toasts';
