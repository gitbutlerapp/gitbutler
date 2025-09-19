<footer class="footer text-12">
	<span>GitButler © 2025</span>

	<div class="footer-links">
		<a class="footer-link" target="_blank" href="https://docs.gitbutler.com/"> Docs ↗ </a>
		<a class="footer-link" target="_blank" href="https://discord.com/invite/MmFkmaJ42D">
			Discord ↗
		</a>
		<a class="footer-link" href="/downloads">Downloads</a>
	</div>

	<div class="footer-links">
		<a class="footer-link underline" href="https://gitbutler.com/privacy" target="_blank">
			Privacy Policy
		</a>
		<span> | </span>
		<a
			class="footer-link underline"
			href="https://app.termly.io/document/terms-and-conditions/7924c71d-80bb-4444-9381-947237b9fc0f"
			target="_blank"
		>
			Terms of Service
		</a>
	</div>
</footer>

<style lang="postcss">
	.footer {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		justify-content: flex-end;
		margin-top: 20px;
		gap: 12px 24px;
		color: var(--clr-text-2);
	}

	.footer-links {
		display: flex;
		align-items: center;
		gap: 8px;
	}

	.footer-link {
		display: flex;
		align-items: center;
		gap: 2px;
		color: var(--clr-text-2);
		transition: color var(--transition-fast);

		&:hover {
			color: var(--clr-text-1);
		}
	}
</style>
