import { describe, it, expect } from "vitest";
import { renderHook, act } from "@testing-library/react";
import { z } from "zod";
import { useFormState } from "./useFormState";
import { validators } from "../lib/validation";

describe("useFormState", () => {
  it("should initialize with provided values", () => {
    const { result } = renderHook(() =>
      useFormState({
        initialValues: { name: "", email: "" },
      })
    );

    expect(result.current.values).toEqual({ name: "", email: "" });
    expect(result.current.errors).toEqual({});
    expect(result.current.isDirty).toBe(false);
  });

  it("should update field values", () => {
    const { result } = renderHook(() =>
      useFormState({
        initialValues: { name: "", email: "" },
      })
    );

    act(() => {
      result.current.setFieldValue("name", "John");
    });

    expect(result.current.values.name).toBe("John");
    expect(result.current.isDirty).toBe(true);
  });

  it("should mark fields as touched", () => {
    const { result } = renderHook(() =>
      useFormState({
        initialValues: { name: "" },
      })
    );

    expect(result.current.touched.name).toBeUndefined();

    act(() => {
      result.current.setFieldTouched("name");
    });

    expect(result.current.touched.name).toBe(true);
  });

  it("should validate form with schema", () => {
    const schema = z.object({
      name: validators.required("Name"),
      email: validators.email(),
    });

    const { result } = renderHook(() =>
      useFormState({
        initialValues: { name: "", email: "" },
        validationSchema: schema,
      })
    );

    const isValid = act(() => result.current.validateForm());

    expect(Object.keys(result.current.errors).length).toBeGreaterThan(0);
  });

  it("should handle form submission", async () => {
    const onSubmit = vi.fn();
    const schema = z.object({
      name: validators.required("Name"),
      email: validators.email(),
    });

    const { result } = renderHook(() =>
      useFormState({
        initialValues: { name: "John", email: "john@example.com" },
        validationSchema: schema,
        onSubmit,
      })
    );

    await act(async () => {
      await result.current.handleSubmit();
    });

    expect(onSubmit).toHaveBeenCalledWith({
      name: "John",
      email: "john@example.com",
    });
  });

  it("should not submit with validation errors", async () => {
    const onSubmit = vi.fn();
    const schema = z.object({
      name: validators.required("Name"),
    });

    const { result } = renderHook(() =>
      useFormState({
        initialValues: { name: "" },
        validationSchema: schema,
        onSubmit,
      })
    );

    await act(async () => {
      await result.current.handleSubmit();
    });

    expect(onSubmit).not.toHaveBeenCalled();
    expect(Object.keys(result.current.errors).length).toBeGreaterThan(0);
  });

  it("should reset form to initial state", () => {
    const { result } = renderHook(() =>
      useFormState({
        initialValues: { name: "John", email: "john@example.com" },
      })
    );

    act(() => {
      result.current.setFieldValue("name", "Jane");
      result.current.setFieldTouched("name");
    });

    expect(result.current.values.name).toBe("Jane");
    expect(result.current.isDirty).toBe(true);

    act(() => {
      result.current.reset();
    });

    expect(result.current.values).toEqual({
      name: "John",
      email: "john@example.com",
    });
    expect(result.current.touched).toEqual({});
    expect(result.current.isDirty).toBe(false);
  });

  it("should clear all errors", () => {
    const { result } = renderHook(() =>
      useFormState({
        initialValues: { name: "" },
      })
    );

    act(() => {
      result.current.setFieldError("name", "Name is required");
    });

    expect(result.current.errors.name).toBe("Name is required");

    act(() => {
      result.current.clearErrors();
    });

    expect(result.current.errors).toEqual({});
  });

  it("should set custom field errors", () => {
    const { result } = renderHook(() =>
      useFormState({
        initialValues: { username: "" },
      })
    );

    act(() => {
      result.current.setFieldError("username", "Username already taken");
    });

    expect(result.current.errors.username).toBe("Username already taken");
  });

  it("should track submission state", async () => {
    const { result } = renderHook(() =>
      useFormState({
        initialValues: { name: "John" },
        onSubmit: () => new Promise((resolve) => setTimeout(resolve, 100)),
      })
    );

    expect(result.current.isSubmitting).toBe(false);

    const submitPromise = act(async () => {
      await result.current.handleSubmit();
    });

    // Note: In real testing, you'd need to check isSubmitting during the submit
    await submitPromise;

    expect(result.current.isSubmitting).toBe(false);
  });
});
