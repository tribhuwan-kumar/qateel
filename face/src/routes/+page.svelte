<script lang="ts">
  import { toast } from "svelte-sonner";
  import { fly } from "svelte/transition";
  import { systemState } from "$lib/system";
  import { Card } from "$lib/components/ui/card";
  import LoginForm from "$lib/components/auth/LoginForm.svelte";
  import RegAdminForm from "$lib/components/auth/RegAdminForm.svelte";
  import ThemeToggle from "@/components/ThemeToggle.svelte";
	import PacmanLoader from "$lib//components/PacmanLoader.svelte";

  let loading = $state(true);
  let showAdminRegForm = $state(false);
  let showSuccessMessage = $state(false);

  function handleSetupComplete() {
    showAdminRegForm = false;
    showSuccessMessage = true;
    if (showSuccessMessage) {
      toast.success("Administration account created", {
        description: "Please proceed to log in!!",
        richColors: true,
        style: "cursor: pointer;"
      })
    }
  }

  /* svelte reactive */
	$effect(() => {
		if ($systemState.status) {
			showAdminRegForm = !$systemState.status.adminExists;
			loading = false;
		}
	});

	let isShowAdminRegForm = $derived(showAdminRegForm);

	$effect(() => {
		console.log("Show admin register:", !isShowAdminRegForm);
	});
</script>

<div class="min-h-screen flex items-center justify-center p-4">
  {#if loading}
		<PacmanLoader />
  {:else}
    <div in:fly={{ y: 100, duration: 800 }}>
      <Card class="md:w-96 w-[clamp(250px, 50vw, 400px)] border-t-2/40 shadow-[0_10px_15px_-3px_rgba(0,0,0,0.1),0_-2px_0_0_var(--muted-foreground)]">
        {#if showAdminRegForm}
					<RegAdminForm
						oncomplete={handleSetupComplete}
					/>
        {:else}
          <LoginForm />
        {/if}
      </Card>
    </div>
		<div
			in:fly={{ y: 100, duration: 800 }}
			class="fixed bottom-8 md:bottom-8 md:left-8 z-50"
		>
			<ThemeToggle />
		</div>
  {/if}
</div>

