import type { MessageStyle } from '$lib/components/InfoMessage.svelte';

export interface Toast {
  id?: string;
  message?: string;
  error?: any;
  title?: string;
  style?: MessageStyle;
}

export function createToaster() {
  let idCounter = 0;
  let toasts: Toast[] = $state([]);

  return {
    get toasts() {
      return toasts;
    },
    showToast: function(toast: Toast) {
      toast.message = toast.message?.replace(/^ */gm, '');
      toasts = [
        ...toasts.filter((t) => toast.id === undefined || t.id !== toast.id),
        { id: (idCounter++).toString(), ...toast }
      ];
    },
    showError: function(title: string, error: any) {
      // Silence GitHub octokit.js when disconnected
      if (error.status === 500 && error.message === 'Load failed') return;

      const message = error.message || error.toString();
      this.showToast({ title, error: message, style: 'error' });
    },

    showInfo: function(title: string, message: string) {
      this.showToast({ title, message, style: 'neutral' });
    },
    dismissToast: function(messageId: string | undefined) {
      if (!messageId) return;
      toasts = toasts.filter((m) => m.id !== messageId);
    }
  };
}
