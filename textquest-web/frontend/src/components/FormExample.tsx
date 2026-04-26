/**
 * FormExample Component
 * Demonstrates form handling with validation using useFormState and validators
 */

import { z } from "zod";
import { Form, Input, Select, TextArea, FormGroup } from "./Form";
import { useFormState } from "../hooks/useFormState";
import { validators, createPasswordMatchValidator } from "../lib/validation";
import { Button } from "./Button";

// Define the validation schema
const registrationSchema = createPasswordMatchValidator(
  z.object({
    username: validators.required("Username").min(3, "Username must be at least 3 characters"),
    email: validators.email(),
    age: validators.positiveNumber("Age"),
    website: validators.url().optional().or(z.literal("")),
    bio: validators.text(0, 500, "Bio").optional().or(z.literal("")),
    country: validators.required("Country"),
    password: validators.password(8),
    confirmPassword: z.string(),
  })
);

type RegistrationFormData = z.infer<typeof registrationSchema>;

const countryOptions = [
  { value: "us", label: "United States" },
  { value: "ca", label: "Canada" },
  { value: "uk", label: "United Kingdom" },
  { value: "au", label: "Australia" },
  { value: "other", label: "Other" },
];

export function FormExample() {
  const form = useFormState<RegistrationFormData>({
    initialValues: {
      username: "",
      email: "",
      age: "",
      website: "",
      bio: "",
      country: "",
      password: "",
      confirmPassword: "",
    },
    validationSchema: registrationSchema,
    onSubmit: async (values) => {
      console.log("Form submitted with values:", values);
      // Here you would typically send data to your API
      // await api.register(values);
    },
  });

  return (
    <Form
      onSubmit={form.handleSubmit}
      isSubmitting={form.isSubmitting}
      className="max-w-md mx-auto p-6"
    >
      <FormGroup>
        <Input
          id="username"
          label="Username"
          type="text"
          placeholder="Enter your username"
          required
          value={form.values.username}
          onChange={(e) => form.setFieldValue("username", e.target.value)}
          onBlur={() => form.setFieldTouched("username")}
          error={form.touched.username ? form.errors.username : ""}
          helperText="Username must be at least 3 characters"
        />

        <Input
          id="email"
          label="Email Address"
          type="email"
          placeholder="your@email.com"
          required
          value={form.values.email}
          onChange={(e) => form.setFieldValue("email", e.target.value)}
          onBlur={() => form.setFieldTouched("email")}
          error={form.touched.email ? form.errors.email : ""}
        />

        <Input
          id="age"
          label="Age"
          type="number"
          required
          value={form.values.age}
          onChange={(e) => form.setFieldValue("age", e.target.value)}
          onBlur={() => form.setFieldTouched("age")}
          error={form.touched.age ? form.errors.age : ""}
        />

        <Input
          id="website"
          label="Website (Optional)"
          type="url"
          placeholder="https://example.com"
          value={form.values.website}
          onChange={(e) => form.setFieldValue("website", e.target.value)}
          onBlur={() => form.setFieldTouched("website")}
          error={form.touched.website ? form.errors.website : ""}
        />

        <TextArea
          id="bio"
          label="Bio (Optional)"
          placeholder="Tell us about yourself..."
          maxLength={500}
          value={form.values.bio}
          onChange={(e) => form.setFieldValue("bio", e.target.value)}
          onBlur={() => form.setFieldTouched("bio")}
          error={form.touched.bio ? form.errors.bio : ""}
        />

        <Select
          id="country"
          label="Country"
          required
          options={countryOptions}
          placeholder="Select your country"
          value={form.values.country}
          onChange={(e) => form.setFieldValue("country", e.target.value)}
          onBlur={() => form.setFieldTouched("country")}
          error={form.touched.country ? form.errors.country : ""}
        />

        <Input
          id="password"
          label="Password"
          type="password"
          required
          placeholder="Enter a strong password"
          value={form.values.password}
          onChange={(e) => form.setFieldValue("password", e.target.value)}
          onBlur={() => form.setFieldTouched("password")}
          error={form.touched.password ? form.errors.password : ""}
          helperText="Must contain uppercase, lowercase, number, and be at least 8 characters"
        />

        <Input
          id="confirmPassword"
          label="Confirm Password"
          type="password"
          required
          placeholder="Re-enter your password"
          value={form.values.confirmPassword}
          onChange={(e) => form.setFieldValue("confirmPassword", e.target.value)}
          onBlur={() => form.setFieldTouched("confirmPassword")}
          error={form.touched.confirmPassword ? form.errors.confirmPassword : ""}
        />
      </FormGroup>

      <div className="flex gap-2 pt-4">
        <Button type="submit" disabled={form.isSubmitting}>
          {form.isSubmitting ? "Submitting..." : "Register"}
        </Button>
        <Button type="button" variant="secondary" onClick={() => form.reset()}>
          Reset
        </Button>
      </div>
    </Form>
  );
}

/**
 * USAGE GUIDE
 *
 * 1. Define your form shape with Zod schema
 * 2. Use validators from lib/validation for common field types
 * 3. Create a custom validator with createCustomValidator for complex rules
 * 4. Pass schema to useFormState hook
 * 5. Connect form components with state and error handling
 * 6. Form automatically validates on submission and field blur
 *
 * Available validators:
 * - validators.required(fieldName)
 * - validators.email()
 * - validators.number(fieldName)
 * - validators.positiveNumber(fieldName)
 * - validators.url()
 * - validators.text(min, max, fieldName)
 * - validators.password(minLength)
 * - validators.phone()
 * - validators.date()
 *
 * Custom validation example:
 * const mySchema = z.object({
 *   age: validators.positiveNumber("Age").refine(
 *     (age) => age >= 18,
 *     "Must be 18 or older"
 *   ),
 * });
 */
