import { z } from "zod";

/**
 * Validation utilities for form handling
 * Provides type-safe, composable validators
 */

// Core field validators
export const validators = {
  /** Validates required string fields */
  required: (fieldName: string = "Field") =>
    z.string().min(1, `${fieldName} is required`),

  /** Validates email format */
  email: () =>
    z.string().email("Invalid email address").min(1, "Email is required"),

  /** Validates number fields */
  number: (fieldName: string = "Number") =>
    z.coerce
      .number()
      .refine((n) => !isNaN(n), `${fieldName} must be a valid number`),

  /** Validates positive numbers */
  positiveNumber: (fieldName: string = "Number") =>
    z.coerce
      .number()
      .positive(`${fieldName} must be greater than 0`)
      .refine((n) => !isNaN(n), `${fieldName} must be a valid number`),

  /** Validates URL format */
  url: () =>
    z.string().url("Invalid URL").min(1, "URL is required"),

  /** Validates text with min/max length */
  text: (min: number = 1, max?: number, fieldName: string = "Text") => {
    let schema = z.string().min(min, `${fieldName} must be at least ${min} characters`);
    if (max) {
      schema = schema.max(max, `${fieldName} must be at most ${max} characters`);
    }
    return schema;
  },

  /** Validates password strength */
  password: (minLength: number = 8) =>
    z
      .string()
      .min(minLength, `Password must be at least ${minLength} characters`)
      .refine(
        (pwd) => /[A-Z]/.test(pwd),
        "Password must contain at least one uppercase letter"
      )
      .refine(
        (pwd) => /[a-z]/.test(pwd),
        "Password must contain at least one lowercase letter"
      )
      .refine(
        (pwd) => /[0-9]/.test(pwd),
        "Password must contain at least one number"
      ),

  /** Validates phone number (simple format) */
  phone: () =>
    z
      .string()
      .regex(/^\+?[1-9]\d{1,14}$/, "Invalid phone number format"),

  /** Validates date string */
  date: () =>
    z.string().refine((date) => !isNaN(Date.parse(date)), "Invalid date format"),
};

/**
 * Validates a single field value against a Zod schema
 * Returns validation error message or null
 */
export function validateField<T>(
  schema: z.ZodType<T>,
  value: unknown
): string | null {
  const result = schema.safeParse(value);
  if (!result.success) {
    return result.error.errors[0]?.message || "Validation failed";
  }
  return null;
}

/**
 * Validates an object against a Zod schema
 * Returns error map keyed by field name
 */
export function validateObject<T extends Record<string, unknown>>(
  schema: z.ZodType<T>,
  data: unknown
): Record<string, string> {
  const result = schema.safeParse(data);
  if (!result.success) {
    const errors: Record<string, string> = {};
    for (const error of result.error.errors) {
      const key = error.path.join(".");
      errors[key] = error.message;
    }
    return errors;
  }
  return {};
}

/**
 * Create a custom validator using a predicate function
 * Useful for cross-field validation or complex rules
 */
export function createCustomValidator<T>(
  schema: z.ZodType<T>,
  predicate: (value: T) => boolean,
  message: string
): z.ZodType<T> {
  return schema.refine(predicate, message);
}

/**
 * Example: Password confirmation validator
 * Used to ensure two password fields match
 */
export function createPasswordMatchValidator(
  schema: z.ZodObject<{ password: any; confirmPassword: any }>
) {
  return schema.refine((data) => data.password === data.confirmPassword, {
    message: "Passwords do not match",
    path: ["confirmPassword"],
  });
}
