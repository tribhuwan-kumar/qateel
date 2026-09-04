<script>
  import { page } from '$app/state';
	import Seriously from "../assets/img/seriously.jpg"
</script>

{#if page.status === 404}
  <div class="error-page">
    <img
			alt="404"
      src={Seriously}
      class="error-image sm:w-[auto] w-[80%]"
    />

    <p class="error-text text-xl sm:text-[clamp(1.5rem,3vw,2.25rem)]">
      Seriously, on 404!!??
    </p>

    <div class="error-button">
      <a
        href="/"
        class="text-md text-muted-foreground cursor-pointer text-center relative
					before:absolute before:bottom-0 before:left-0 before:h-[2px] before:w-full before:origin-right
					before:scale-x-0 before:bg-muted-foreground/60 before:rounded-full before:transition-transform before:duration-300
					before:ease-in-out hover:before:origin-left hover:before:scale-x-100"
      >
        Go home
      </a>
    </div>
  </div>
{/if}

<style>
  .error-page {
    width: 100%;
    min-height: 100svh;
    display: flex;
    flex-direction: column;
    justify-content: center;
    align-items: center;
    gap: 1.5rem;
    background: var(--background);
    color: var(--foreground);
    overflow: hidden;
  }

  .error-image {
    padding: 0.75rem;
    border-radius: 1rem;
    border: 3px solid color-mix(in oklch, var(--primary) 75%, white);
    background: color-mix(in oklch, var(--card) 92%, transparent);
    box-shadow:
      0 0 0 1px color-mix(in oklch, var(--border) 70%, transparent),
      0 0 24px color-mix(in oklch, var(--primary) 35%, transparent),
      0 0 48px color-mix(in oklch, var(--ring) 20%, transparent);
    animation: floaty 3.5s ease-in-out infinite,
               glowPulse 2.6s ease-in-out infinite;
  }

  .error-text {
    /* font-size: clamp(1.5rem, 3vw, 2.25rem); */
    font-weight: 700;
    letter-spacing: 0.02em;
    color: var(--foreground);
    text-shadow:
      0 0 10px color-mix(in oklch, var(--primary) 18%, transparent),
      0 0 24px color-mix(in oklch, var(--ring) 14%, transparent);
    animation: textPulse 2.8s ease-in-out infinite;
  }

  .error-button {
    filter: drop-shadow(0 0 10px color-mix(in oklch, var(--primary) 18%, transparent));
    transition: transform 0.2s ease, filter 0.2s ease;
  }

  .error-button:hover {
    transform: translateY(-2px) scale(1.03);
    filter: drop-shadow(0 0 16px color-mix(in oklch, var(--primary) 35%, transparent));
  }

  @keyframes floaty {
    0%, 100% {
      transform: translateY(0);
    }
    50% {
      transform: translateY(-8px);
    }
  }

  @keyframes glowPulse {
    0%, 100% {
      box-shadow:
        0 0 0 1px color-mix(in oklch, var(--border) 70%, transparent),
        0 0 24px color-mix(in oklch, var(--primary) 35%, transparent),
        0 0 48px color-mix(in oklch, var(--ring) 20%, transparent);
    }
    50% {
      box-shadow:
        0 0 0 1px color-mix(in oklch, var(--border) 70%, transparent),
        0 0 32px color-mix(in oklch, var(--primary) 50%, transparent),
        0 0 60px color-mix(in oklch, var(--ring) 30%, transparent);
    }
  }

  @keyframes textPulse {
    0%, 100% {
      opacity: 0.92;
      transform: scale(1);
    }
    50% {
      opacity: 1;
      transform: scale(1.02);
    }
  }
</style>

