<script lang="ts">
	import FileStatusCircle from './FileStatusCircle.svelte';
	import { computeFileStatus } from '$lib/utils/fileStatus';
	import { getLocalCommits, getLocalAndRemoteCommits } from '$lib/vbranches/contexts';
	import { getLockText } from '$lib/vbranches/tooltip';
	import { type AnyFile, LocalFile } from '$lib/vbranches/types';
	import Icon from '@gitbutler/ui/icon/Icon.svelte';
	import { tooltip } from '@gitbutler/ui/utils/tooltip';

	export let file: AnyFile;

	// TODO: Refactor this into something more meaningful.
	const localCommits = file instanceof LocalFile ? getLocalCommits() : undefined;
	const remoteCommits = file instanceof LocalFile ? getLocalAndRemoteCommits() : undefined;

	$: lockedIds = file.lockedIds;
	$: lockText =
		lockedIds.length > 0 && $localCommits
			? getLockText(lockedIds, ($localCommits || []).concat($remoteCommits || []))
			: '';
</script>

<div class="file-status">
	<div class="file-status__icons">
		{#if lockText}
			<div class="locked" use:tooltip={{ text: lockText, delay: 500 }}>
				<Icon name="locked-small" color="warning" />
			</div>
		{/if}
		{#if file.conflicted}
			<div class="conflicted">
				<Icon name="warning-small" color="error" />
			</div>
		{/if}
	</div>
	<div class="status">
		<FileStatusCircle status={computeFileStatus(file)} />
	</div>
</div>

<style lang="postcss">
	.file-status {
		display: flex;
		align-items: center;
		gap: 4px;
	}
	.file-status__icons {
		display: flex;
	}
</style>
