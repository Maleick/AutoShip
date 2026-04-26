import { useState, useCallback } from "react";
import { z } from "zod";
import { validateObject } from "../lib/validation";

/**
 * Form state and validation hook
 * Manages form values, errors, and submission state
 */
export interface FormState<T extends Record<string, unknown>> {
  values: T;
  errors: Record<string, string>;
  touched: Record<string, boolean>;
  isSubmitting: boolean;
  isDirty: boolean;
}

export interface UseFormStateOptions<T extends Record<string, unknown>> {
  initialValues: T;
  validationSchema?: z.ZodType<T>;
  onSubmit?: (values: T) => void | Promise<void>;
}

export function useFormState<T extends Record<string, unknown>>({
  initialValues,
  validationSchema,
  onSubmit,
}: UseFormStateOptions<T>) {
  const [state, setState] = useState<FormState<T>>({
    values: initialValues,
    errors: {},
    touched: {},
    isSubmitting: false,
    isDirty: false,
  });

  /**
   * Set a field value and mark it as touched
   */
  const setFieldValue = useCallback((name: string, value: unknown) => {
    setState((prev) => ({
      ...prev,
      values: {
        ...prev.values,
        [name]: value,
      },
      isDirty: true,
    }));
  }, []);

  /**
   * Set a field as touched (usually on blur)
   */
  const setFieldTouched = useCallback((name: string) => {
    setState((prev) => ({
      ...prev,
      touched: {
        ...prev.touched,
        [name]: true,
      },
    }));
  }, []);

  /**
   * Validate a single field
   */
  const validateField = useCallback(
    (name: string) => {
      if (!validationSchema) return null;

      const fieldSchema = (validationSchema as any)._shape?.[name];
      if (!fieldSchema) return null;

      const result = fieldSchema.safeParse(state.values[name]);
      if (!result.success) {
        const error = result.error.errors[0]?.message;
        setState((prev) => ({
          ...prev,
          errors: {
            ...prev.errors,
            [name]: error || "Invalid input",
          },
        }));
        return error || "Invalid input";
      } else {
        setState((prev) => ({
          ...prev,
          errors: {
            ...prev.errors,
            [name]: "",
          },
        }));
        return null;
      }
    },
    [state.values, validationSchema]
  );

  /**
   * Validate all fields
   */
  const validateForm = useCallback((): boolean => {
    if (!validationSchema) return true;

    const errors = validateObject(validationSchema, state.values);
    setState((prev) => ({
      ...prev,
      errors,
      touched: Object.keys(state.values).reduce(
        (acc, key) => ({
          ...acc,
          [key]: true,
        }),
        {}
      ),
    }));

    return Object.keys(errors).length === 0;
  }, [state.values, validationSchema]);

  /**
   * Handle form submission
   */
  const handleSubmit = useCallback(
    async (e?: React.FormEvent) => {
      e?.preventDefault();

      if (!validateForm()) {
        return;
      }

      setState((prev) => ({
        ...prev,
        isSubmitting: true,
      }));

      try {
        if (onSubmit) {
          await onSubmit(state.values);
        }
      } finally {
        setState((prev) => ({
          ...prev,
          isSubmitting: false,
        }));
      }
    },
    [state.values, validateForm, onSubmit]
  );

  /**
   * Reset form to initial state
   */
  const reset = useCallback(() => {
    setState({
      values: initialValues,
      errors: {},
      touched: {},
      isSubmitting: false,
      isDirty: false,
    });
  }, [initialValues]);

  /**
   * Clear all errors
   */
  const clearErrors = useCallback(() => {
    setState((prev) => ({
      ...prev,
      errors: {},
    }));
  }, []);

  /**
   * Set custom error message for a field
   */
  const setFieldError = useCallback((name: string, error: string) => {
    setState((prev) => ({
      ...prev,
      errors: {
        ...prev.errors,
        [name]: error,
      },
    }));
  }, []);

  return {
    ...state,
    setFieldValue,
    setFieldTouched,
    validateField,
    validateForm,
    handleSubmit,
    reset,
    clearErrors,
    setFieldError,
  };
}
