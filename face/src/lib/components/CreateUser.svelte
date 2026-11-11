<script lang="ts">
  import { toast } from "svelte-sonner";
  import { slide } from 'svelte/transition';
  import { Button } from "$lib/components/ui/button";
	import { authState } from "$lib/auth/auth.svelte";
  import { Label } from "$lib/components/ui/label";
  import { HugeiconsIcon } from "@hugeicons/svelte";
	import { isValidPassword, isValidUsername } from "$lib/utils";
	import { LoaderPinwheelIcon, } from '@hugeicons/core-free-icons';
  import * as Dialog from '$lib/components/ui/dialog';
  let {
		open = $bindable(false),
  } = $props<{
    open: boolean;
  }>();

  let error = $state("");
  let passStep = $state(1);
  let creatingUsername = $state("");
  let creatingUserPassword = $state("");
  let reinputCreatingUserPassword = $state("");
  let loading = $state(false);
	let requesterUsername = $derived(authState.username);

  async function handleCreatingUser(event: Event) {
    event.preventDefault();
    error = "";

    if (creatingUsername.length < 6) {
      error = "Username must be at least 6 characters!!";
      loading = false;
      toast.error(error, {
        richColors: true,
        style: "cursor: pointer;"
      })
      return;
    }

    if (!isValidUsername(creatingUsername)) {
      error = "Username can only contain letters, numbers, and underscores!!";
      loading = false;
      toast.error(error, {
        richColors: true,
        style: "cursor: pointer;"
      })
      return;
    }

    if (passStep === 1) {
      if (!isValidPassword(creatingUserPassword)) {
        error = "Password must be at least 8 characters!!";
        loading = false;
        toast.error(error, {
          richColors: true,
          style: "cursor: pointer;"
        })
        return;
      }
      passStep = 2;
      return;
    }

    if (passStep === 2) {
      if (creatingUserPassword !== reinputCreatingUserPassword) {
        error = "Passwords do not match.";
        toast.error(error, {
          richColors: true,
          style: "cursor: pointer;"
        })
        return;
      }
      loading = true;
      try {
        const res = await fetch('/api/auth/create/user', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({
						username: creatingUsername,
						password: creatingUserPassword,
						requesterUsername: requesterUsername
					})
        });
        const data = await res.json();
        if (!res.ok) throw new Error(data.error || "Failed to create user.");
				open = false;
        toast.success(`User created: ${creatingUsername}`, {
          richColors: true,
          style: "cursor: pointer;"
        })
      } catch (e: any) {
        toast.error("Error while creating user:", {
          description: e.message,
          richColors: true,
          style: "cursor: pointer;"
        })
      } finally {
        loading = false;
      }
    }
  }
</script>

<Dialog.Root
	bind:open
	onOpenChange={(v) => {
		open = v
		passStep = 1
		creatingUserPassword = ""
		reinputCreatingUserPassword = ""
	}}
>
	<Dialog.Content class="py-10 px-1! flex flex-col justify-center items-center" showCloseButton={false}>
		<Dialog.Header>
			<Dialog.Title class="text-2xl font-bold text-center">Create a user</Dialog.Title>
		</Dialog.Header>
		<form onsubmit={handleCreatingUser}
			class="space-y-4 flex w-[80%] flex-col justify-center items-center">
			<div class="space-y-2">
				<Label for="username" class="text-muted-foreground text-[14px] ml-1">Username</Label>
				<input
					id="username"
					type="text"
					autocomplete="off"
					autocorrect="off"
					autocapitalize="off"
					bind:value={creatingUsername}
					disabled={loading || passStep === 2}
					class="disabled:cursor-not-allowed disabled:pointer-events-auto! w-full ml-[2px] rounded-2xl border-2
						border-sidebar-border/60 bg-transparent px-2 py-1.5 outline-none placeholder:text-muted-foreground/50
						text-foreground transition-shadow focus-visible:ring-2 focus-visible:ring-sidebar-border"
					required
				/>
			</div>

			<div class="overflow-hidden h-16">
				{#if passStep === 1}
					<div class="space-y-2"
							out:slide={{ axis: 'x', duration: 400 }}
							onoutroend={() => passStep = 2}
						>
						<Label for="password" class="text-muted-foreground text-[14px] ml-1">Password</Label>
						<input
							id="password"
							type="password"
							autocomplete="off"
							autocorrect="off"
							autocapitalize="off"
							bind:value={creatingUserPassword}
							disabled={loading}
							class="disabled:cursor-not-allowed! w-full ml-[2px] rounded-2xl border-2 border-sidebar-border/60
								bg-transparent px-2 py-1.5 outline-none placeholder:text-muted-foreground/50 text-foreground transition-shadow
								focus-visible:ring-2 focus-visible:ring-sidebar-border"
							required
						/>
					</div>
				{:else if passStep === 2}
					<div
							class="space-y-2"
							in:slide={{ axis: 'x', duration: 600, delay: 500 }}
						>
						<Label for="reinputPassword" class="text-muted-foreground text-[14px] ml-1">Type again</Label>
						<input
							id="reinputPassword"
							type="password"
							autocomplete="off"
							autocorrect="off"
							autocapitalize="off"
							bind:value={reinputCreatingUserPassword}
							disabled={loading}
							class="disabled:cursor-not-allowed!
								w-full ml-[2px] rounded-2xl border-2 border-sidebar-border/60 bg-transparent px-2 py-1.5
								outline-none placeholder:text-muted-foreground/50 text-foreground transition-shadow focus-visible:ring-2 focus-visible:ring-sidebar-border"
							required
						/>
					</div>
				{/if}
			</div>
			<Button
				type="submit"
				class="mt-2 md:w-[75%]! sm:w-[75%]! w-[80%] cursor-pointer disabled:cursor-not-allowed!"
				disabled={loading}
			>
				{#if loading}
					<HugeiconsIcon
						icon={LoaderPinwheelIcon}
						class="w-4 h-4 mr-2"
						strokeWidth={2}
						size={24}
					/>
						Creating...
					{:else if passStep === 1}
						Next
					{:else}
						Create account
					{/if}
				</Button>
		</form>
		<Dialog.Footer
			class="flex items-center justify-center! text-xs text-center text-muted-foreground">
			You can create only<span class="italic">non admin</span>user!!
		</Dialog.Footer>
	</Dialog.Content>
</Dialog.Root>

