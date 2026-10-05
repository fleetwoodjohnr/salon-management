// React Query helpers shared by features. Query keys are the command name plus arguments.
import { notifications } from "@mantine/notifications";
import { useMutation, useQuery, useQueryClient, type QueryKey } from "@tanstack/react-query";
import { call, errorMessage } from "./ipc";

export function useCmd<T>(cmd: string, args?: Record<string, unknown>, enabled = true) {
  return useQuery<T>({ queryKey: [cmd, args ?? {}], queryFn: () => call<T>(cmd, args), enabled });
}

/**
 * Mutation that invalidates the given query-key prefixes on success and shows a toast.
 * Errors surface as a red toast unless `silentError` (forms show field errors inline instead).
 */
export function useAction<TArgs extends Record<string, unknown>, TOut = unknown>(
  cmd: string,
  opts: { invalidate?: string[]; success?: string | ((out: TOut) => string); silentError?: boolean } = {},
) {
  const qc = useQueryClient();
  return useMutation<TOut, unknown, TArgs>({
    mutationFn: (args) => call<TOut>(cmd, args),
    onSuccess: (out) => {
      for (const k of opts.invalidate ?? []) qc.invalidateQueries({ queryKey: [k] as QueryKey });
      const msg = typeof opts.success === "function" ? opts.success(out) : opts.success;
      if (msg) notifications.show({ message: msg, color: "teal" });
    },
    onError: (e) => {
      if (!opts.silentError) notifications.show({ title: "Not saved", message: errorMessage(e), color: "red", autoClose: 8000 });
    },
  });
}
