import {
	AUTHOR_IDENTITY_SERVICE,
	fallbackAuthorIdentity,
	type AuthorIdentityInput,
	type ResolveAuthorIdentityOptions,
	type ResolvedAuthorIdentity,
} from "$lib/user/authorIdentityService"
import { inject } from "@gitbutler/core/context"
import { reactive } from "@gitbutler/shared/reactiveUtils.svelte"
import type { Reactive } from "@gitbutler/shared/storeUtils"

export function useResolvedAuthorIdentity(
	author: Reactive<AuthorIdentityInput | undefined>,
	options?: Reactive<ResolveAuthorIdentityOptions | undefined>,
): Reactive<ResolvedAuthorIdentity | undefined> {
	const authorIdentityService = inject(AUTHOR_IDENTITY_SERVICE)

	let identity = $state<ResolvedAuthorIdentity | undefined>(fallbackAuthorIdentity(author.current))

	$effect(() => {
		const currentAuthor = author.current
		const currentOptions = options?.current

		identity = fallbackAuthorIdentity(currentAuthor)

		let cancelled = false
		void authorIdentityService.resolve(currentAuthor, currentOptions).then((resolvedIdentity) => {
			if (!cancelled) {
				identity = resolvedIdentity
			}
		})

		return () => {
			cancelled = true
		}
	})

	return reactive(() => identity)
}

export function useResolvedAuthorIdentities(
	authors: Reactive<ReadonlyArray<AuthorIdentityInput> | undefined>,
): Reactive<ResolvedAuthorIdentity[]> {
	const authorIdentityService = inject(AUTHOR_IDENTITY_SERVICE)

	let identities = $state<ResolvedAuthorIdentity[]>(
		(authors.current ?? [])
			.map((author) => fallbackAuthorIdentity(author))
			.filter((author): author is ResolvedAuthorIdentity => !!author),
	)

	$effect(() => {
		const currentAuthors = authors.current ?? []

		identities = currentAuthors
			.map((author) => fallbackAuthorIdentity(author))
			.filter((author): author is ResolvedAuthorIdentity => !!author)

		let cancelled = false
		void authorIdentityService.resolveMany(currentAuthors).then((resolvedIdentities) => {
			if (!cancelled) {
				identities = resolvedIdentities
			}
		})

		return () => {
			cancelled = true
		}
	})

	return reactive(() => identities)
}
