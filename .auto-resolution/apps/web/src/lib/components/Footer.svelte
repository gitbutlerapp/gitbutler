<footer class="footer text-12">
	<span>GitButler © 2025</span>
	<a class="footer-link" target="_blank" href="https://docs.gitbutler.com/"> Docs ↗ </a>
	<a class="footer-link" target="_blank" href="https://discord.com/invite/MmFkmaJ42D">
		Discord ↗
	</a>
	<a class="footer-link" href="/downloads">Downloads</a>
</footer>

<style lang="postcss">
	.footer {
		display: flex;
		align-items: center;
		justify-content: flex-end;
		margin-top: 20px;
		gap: 24px;
		color: var(--clr-text-2);
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
