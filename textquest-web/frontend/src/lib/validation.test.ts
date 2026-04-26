import { describe, it, expect } from "vitest";
import {
  validators,
  validateField,
  validateObject,
  createCustomValidator,
  createPasswordMatchValidator,
} from "./validation";
import { z } from "zod";

describe("validators", () => {
  describe("required", () => {
    it("should validate non-empty strings", () => {
      const schema = validators.required("Username");
      expect(validateField(schema, "john")).toBeNull();
    });

    it("should reject empty strings", () => {
      const schema = validators.required("Username");
      expect(validateField(schema, "")).not.toBeNull();
    });
  });

  describe("email", () => {
    it("should validate valid emails", () => {
      const schema = validators.email();
      expect(validateField(schema, "user@example.com")).toBeNull();
    });

    it("should reject invalid emails", () => {
      const schema = validators.email();
      expect(validateField(schema, "not-an-email")).not.toBeNull();
    });

    it("should reject empty email", () => {
      const schema = validators.email();
      expect(validateField(schema, "")).not.toBeNull();
    });
  });

  describe("number", () => {
    it("should validate numbers", () => {
      const schema = validators.number();
      expect(validateField(schema, 42)).toBeNull();
      expect(validateField(schema, "123")).toBeNull();
    });

    it("should reject non-numbers", () => {
      const schema = validators.number();
      expect(validateField(schema, "abc")).not.toBeNull();
    });
  });

  describe("positiveNumber", () => {
    it("should validate positive numbers", () => {
      const schema = validators.positiveNumber();
      expect(validateField(schema, 42)).toBeNull();
    });

    it("should reject zero", () => {
      const schema = validators.positiveNumber();
      expect(validateField(schema, 0)).not.toBeNull();
    });

    it("should reject negative numbers", () => {
      const schema = validators.positiveNumber();
      expect(validateField(schema, -5)).not.toBeNull();
    });
  });

  describe("url", () => {
    it("should validate valid URLs", () => {
      const schema = validators.url();
      expect(validateField(schema, "https://example.com")).toBeNull();
    });

    it("should reject invalid URLs", () => {
      const schema = validators.url();
      expect(validateField(schema, "not a url")).not.toBeNull();
    });
  });

  describe("text", () => {
    it("should validate text with length constraints", () => {
      const schema = validators.text(3, 10);
      expect(validateField(schema, "hello")).toBeNull();
    });

    it("should reject text below minimum length", () => {
      const schema = validators.text(5);
      expect(validateField(schema, "hi")).not.toBeNull();
    });

    it("should reject text above maximum length", () => {
      const schema = validators.text(3, 5);
      expect(validateField(schema, "toolong")).not.toBeNull();
    });
  });

  describe("password", () => {
    it("should validate strong passwords", () => {
      const schema = validators.password();
      expect(validateField(schema, "SecurePass123")).toBeNull();
    });

    it("should reject password without uppercase", () => {
      const schema = validators.password();
      expect(validateField(schema, "securepass123")).not.toBeNull();
    });

    it("should reject password without lowercase", () => {
      const schema = validators.password();
      expect(validateField(schema, "SECUREPASS123")).not.toBeNull();
    });

    it("should reject password without number", () => {
      const schema = validators.password();
      expect(validateField(schema, "SecurePass")).not.toBeNull();
    });

    it("should reject password below minimum length", () => {
      const schema = validators.password(8);
      expect(validateField(schema, "Pass123")).not.toBeNull();
    });
  });

  describe("phone", () => {
    it("should validate valid phone numbers", () => {
      const schema = validators.phone();
      expect(validateField(schema, "14155552671")).toBeNull();
      expect(validateField(schema, "+14155552671")).toBeNull();
    });

    it("should reject invalid phone numbers", () => {
      const schema = validators.phone();
      expect(validateField(schema, "123")).not.toBeNull();
      expect(validateField(schema, "abc123")).not.toBeNull();
    });
  });

  describe("date", () => {
    it("should validate valid dates", () => {
      const schema = validators.date();
      expect(validateField(schema, "2024-01-01")).toBeNull();
      expect(validateField(schema, "01/01/2024")).toBeNull();
    });

    it("should reject invalid dates", () => {
      const schema = validators.date();
      expect(validateField(schema, "not a date")).not.toBeNull();
    });
  });
});

describe("validateObject", () => {
  it("should validate entire objects", () => {
    const schema = z.object({
      email: validators.email(),
      username: validators.required("Username"),
    });

    const errors = validateObject(schema, {
      email: "user@example.com",
      username: "john",
    });

    expect(Object.keys(errors)).toHaveLength(0);
  });

  it("should collect multiple errors", () => {
    const schema = z.object({
      email: validators.email(),
      username: validators.required("Username"),
    });

    const errors = validateObject(schema, {
      email: "invalid-email",
      username: "",
    });

    expect(Object.keys(errors).length).toBeGreaterThan(0);
    expect(errors.email).toBeDefined();
    expect(errors.username).toBeDefined();
  });
});

describe("createCustomValidator", () => {
  it("should validate with custom predicate", () => {
    const schema = createCustomValidator(
      z.number(),
      (val) => val % 2 === 0,
      "Must be an even number"
    );

    expect(validateField(schema, 4)).toBeNull();
    expect(validateField(schema, 3)).not.toBeNull();
  });
});

describe("createPasswordMatchValidator", () => {
  it("should validate matching passwords", () => {
    const schema = createPasswordMatchValidator(
      z.object({
        password: validators.password(),
        confirmPassword: z.string(),
      })
    );

    const result = schema.safeParse({
      password: "SecurePass123",
      confirmPassword: "SecurePass123",
    });

    expect(result.success).toBe(true);
  });

  it("should reject non-matching passwords", () => {
    const schema = createPasswordMatchValidator(
      z.object({
        password: validators.password(),
        confirmPassword: z.string(),
      })
    );

    const result = schema.safeParse({
      password: "SecurePass123",
      confirmPassword: "DifferentPass123",
    });

    expect(result.success).toBe(false);
  });
});
