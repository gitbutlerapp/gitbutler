<script lang="ts">
    import SectionCard from "$lib/components/SectionCard.svelte";
	import { KeyOption, ModelKind, getModelKind, getKeyOption, setModelKind, setKeyOption, getAnthropicKey, setAnthropicKey, setOpenAIKey, getOpenAIKey, AnthropicModel, getAnthropicModel, OpenAIModel, getOpenAIModel, setAnthropicModel, setOpenAIModel } from "$lib/backend/summarizer_settings";
	import Select from "./Select.svelte";
	import SelectItem from "./SelectItem.svelte";
	import TextBox from "./TextBox.svelte";

    let modelKind: { name: string, value: ModelKind } | undefined;
    getModelKind().then((kind) => modelKind = modelKinds.find((option) => option.value == kind))
    $: if (modelKind) setModelKind(modelKind.value)

    const modelKinds = [
        {
            name: "Open AI",
            value: ModelKind.OpenAI
        },
        {
            name: "Anthropic",
            value: ModelKind.Anthropic
        }
    ]

    let keyOption: { name: string, value: KeyOption } | undefined;
    getKeyOption().then((persistedKeyOption) => keyOption = keyOptions.find((option) => option.value == persistedKeyOption))
    $: if (keyOption) setKeyOption(keyOption.value)

    const keyOptions = [
        {
            name: "Butler API",
            value: KeyOption.ButlerAPI
        },
        {
            name: "Bring your own key",
            value: KeyOption.BringYourOwn
        }
    ]

    let openAIKey: string | undefined;
    getOpenAIKey().then((persistedOpenAIKey) => openAIKey = persistedOpenAIKey)
    $: if (openAIKey) setOpenAIKey(openAIKey)

    let openAIModel: { name: string, value: OpenAIModel } | undefined;
    getOpenAIModel().then((persistedOpenAIModel) => openAIModel = openAIModelOptions.find((option) => option.value == persistedOpenAIModel))
    $: if (openAIModel) setOpenAIModel(openAIModel.value)

    const openAIModelOptions = [
        {
            name: "GPT 3.5 Turbo",
            value: OpenAIModel.GPT35Turbo
        },
        {
            name: "GPT 4",
            value: OpenAIModel.GPT4
        },
        {
            name: "GPT 4 Turbo",
            value: OpenAIModel.GPT4Turbo
        },
    ]

    let anthropicKey: string | undefined;
    getAnthropicKey().then((persistedAnthropicKey) => anthropicKey = persistedAnthropicKey)
    $: if (anthropicKey) setAnthropicKey(anthropicKey)

    let anthropicModel: { name: string, value: AnthropicModel } | undefined;
    getAnthropicModel().then((persistedAnthropicModel) => anthropicModel = anthropicModelOptions.find((option) => option.value == persistedAnthropicModel))
    $: if (anthropicModel) setAnthropicModel(anthropicModel.value)

    const anthropicModelOptions = [
        {
            name: "Sonnet",
            value: AnthropicModel.Sonnet
        },
        {
            name: "Opus",
            value: AnthropicModel.Opus
        }
    ]
</script>

<SectionCard>
    <svelte:fragment slot="title">Model Kind</svelte:fragment>
    <svelte:fragment slot="body">
        GitButler supports OpenAI and Anthropic for various summerization tasks, either proxied via the GitButler servers or in a bring your own key configuration.
    </svelte:fragment>

    <Select items={modelKinds} bind:value={modelKind} itemId="value" labelId="name">
        <SelectItem slot="template" let:item>
            {item.name}
        </SelectItem>
    </Select>
</SectionCard>

<SectionCard>
    <svelte:fragment slot="title">Key Configuration</svelte:fragment>
    <svelte:fragment slot="body">
        GitButler can either be configured to be proxied via the GitButler servers or to use your own key.
    </svelte:fragment>

    <Select items={keyOptions} bind:value={keyOption} itemId="value" labelId="name">
        <SelectItem slot="template" let:item>
            {item.name}
        </SelectItem>
    </Select>

    {#if keyOption?.value === KeyOption.BringYourOwn}
        {#if modelKind?.value == ModelKind.Anthropic}
            <TextBox label="Anthropic API Key" bind:value={anthropicKey}/>

            <Select items={anthropicModelOptions} bind:value={anthropicModel} itemId="value" labelId="name" label="Model Version">
                <SelectItem slot="template" let:item>
                    {item.name}
                </SelectItem>
            </Select>
        {:else if modelKind?.value == ModelKind.OpenAI}
            <TextBox label="OpenAI API Key" bind:value={openAIKey}/>

            <Select items={openAIModelOptions} bind:value={openAIModel} itemId="value" labelId="name" label="Model Version">
                <SelectItem slot="template" let:item>
                    {item.name}
                </SelectItem>
            </Select>
        {/if}
    {/if}
</SectionCard>
