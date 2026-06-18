import type { EvalCase } from "./lib/harness.ts";
import { textOf } from "./lib/harness.ts";

interface PracticePage {
  slug: string;
  label: string;
  path: string;
  smokeTerms: string[];
  cases: string[];
}

interface DetailedCase {
  pageSlug: string;
  tc: string;
  prefix?: string;
  prompt: string;
  extraConstraints?: string[];
}

interface AutomationExerciseTestCase {
  tc: string;
  title: string;
  steps: string[];
  extraConstraints?: string[];
  fastPath?: string[];
}

const practicePages: PracticePage[] = [
  {
    slug: "bank",
    label: "Bank Login",
    path: "/bank",
    smokeTerms: ["SecureBank", "Login"],
    cases: [
      "TC-LOGIN-01: Successful login with admin credentials",
      "TC-LOGIN-02: Failed login shows error alert for invalid credentials",
      "TC-LOGIN-03: Toggle password visibility hides and reveals password text",
      "TC-LOGIN-04: Pressing Enter in the password field submits the login form",
      "TC-LOGIN-05: Read-only viewer login grants restricted access",
    ],
  },
  {
    slug: "input-fields",
    label: "Input Fields",
    path: "/practice/input-fields",
    smokeTerms: ["input field", "movie name"],
    cases: [
      "TC01: Verify successful movie name input",
      "TC02: Verify input placeholder disappears on typing",
      "TC03: Verify keyboard tab triggers focus change after append",
      "TC04: Verify appended text value is retained in the field",
      "TC05: Verify text present inside input field matches expected value",
      "TC06: Verify getAttribute returns the correct input value",
      "TC07: Verify input field text can be cleared successfully",
      "TC08: Verify field is empty after executing clear action",
      "TC09: Verify disabled input field cannot be edited by user",
      "TC10: Verify isEnabled() returns false for disabled input",
      "TC11: Verify readonly input field does not accept user typing",
      "TC12: Verify getAttribute returns correct readonly attribute value",
    ],
  },
  {
    slug: "buttons",
    label: "Buttons",
    path: "/practice/buttons",
    smokeTerms: ["button", "double click"],
    cases: [
      "TC01: Verify button is clickable and triggers action",
      "TC02: Verify button displays the correct label text",
      "TC03: Verify button triggers the correct action on click",
      "TC04: Verify double-click button triggers double-click action",
      "TC05: Verify right-click button triggers context menu action",
      "TC06: Verify disabled button cannot be clicked",
      "TC07: Verify button is enabled when it should be",
      "TC08: Verify button is responsive on different screen sizes",
      "TC09: Verify button is accessible via keyboard",
      "TC10: Verify button is accessible to screen readers",
      "TC11: Verify button hover state is visually distinct",
      "TC12: Verify button state resets after page refresh",
      "TC13: Verify button does not overlap other page elements",
      "TC14: Verify button styling matches design specification",
      "TC15: Verify button page loads without errors",
    ],
  },
  {
    slug: "forms",
    label: "Forms",
    path: "/practice/forms",
    smokeTerms: ["form", "first name"],
    cases: [
      "TC01: Fill all fields with valid data and submit successfully",
      "TC02: Verify required field errors appear on empty submit",
      "TC03: Verify invalid email format shows validation error",
      "TC04: Verify invalid phone number format shows error",
      "TC05: Verify password minimum length validation",
      "TC06: Verify password mismatch shows confirm password error",
      "TC07: Verify T&C checkbox required error appears",
      "TC08: Verify success message displays submitted name",
      "TC09: Verify reset button clears all fields",
      "TC10: Verify gender radio button selection",
      "TC11: Verify country dropdown selection",
      "TC12: Verify multiple interest checkboxes can be selected",
      "TC13: Verify form fields retain values after validation failure",
      "TC14: Verify Fill Again button returns to empty form from success state",
      "TC15: Verify form page loads without errors",
    ],
  },
  {
    slug: "dropdowns",
    label: "Dropdowns",
    path: "/practice/dropdowns",
    smokeTerms: ["dropdown", "select fruit"],
    cases: [
      "TC01: Select 'Apple' from fruit dropdown by visible text",
      "TC02: Select 'India' from country dropdown by value attribute",
      "TC03: Verify selected value is displayed after selection",
      "TC04: Get all available options from the programming language dropdown",
      "TC05: Select the last option from the programming language dropdown",
      "TC06: Multi-select: select multiple superheroes using CTRL+click",
      "TC07: Multi-select: deselect a previously selected option",
      "TC08: Verify default placeholder text before any selection",
      "TC09: Verify a dropdown is enabled and interactable",
      "TC10: Verify the total count of options in the country dropdown",
    ],
  },
  {
    slug: "data-table",
    label: "Data Table",
    path: "/practice/data-table",
    smokeTerms: ["data table", "book name"],
    cases: [
      "TC01: Verify all table column headers are present",
      "TC02: Count the total number of rows in the data table",
      "TC03: Read a cell value from a specific row and column",
      "TC04: Find a book row by author name using XPath or filter",
      "TC05: Verify the table is not empty after page load",
      "TC06: Assert the ISBN column contains only string values",
    ],
  },
  {
    slug: "alerts-dialogs",
    label: "Alerts & Dialogs",
    path: "/practice/alerts-dialogs",
    smokeTerms: ["alerts", "dialogs"],
    cases: [
      "TC01: Accept a simple browser alert and verify it closes",
      "TC02: Get text from a simple browser alert before accepting",
      "TC03: Accept a confirm dialog and verify accepted state",
      "TC04: Dismiss a confirm dialog and verify dismissed state",
      "TC05: Enter text in a prompt dialog and accept it",
      "TC06: Dismiss a prompt dialog and verify no input is captured",
      "TC07: Verify toast notification appears after triggering",
      "TC08: Close a modal/sweet alert using the Cancel button",
      "TC09: Close an advanced dialog using the Close button",
      "TC10: Verify alerts page loads without errors",
    ],
  },
  {
    slug: "radio-checkbox",
    label: "Radio & Checkbox",
    path: "/practice/radio-checkbox",
    smokeTerms: ["radio", "checkbox"],
    cases: [
      "TC01: Verify radio button is selected on click",
      "TC02: Verify selecting another radio deselects the previous one",
      "TC03: Verify only one radio button can be selected at a time",
      "TC04: Verify radio button label text is correct",
      "TC05: Verify radio button state persists after page interaction",
      "TC06: Verify checkbox can be checked",
      "TC07: Verify checkbox can be unchecked",
      "TC08: Verify multiple checkboxes can be selected simultaneously",
      "TC09: Verify radio buttons are keyboard navigable",
      "TC10: Verify checkbox is keyboard togglable",
      "TC11: Verify disabled radio button cannot be selected",
      "TC12: Verify disabled checkbox cannot be toggled",
      "TC13: Verify radio button group is accessible to screen readers",
      "TC14: Verify radio button visual state changes on selection",
      "TC15: Verify radio and checkbox elements load without errors",
    ],
  },
  {
    slug: "date-picker",
    label: "Date Picker",
    path: "/practice/date-picker",
    smokeTerms: ["date picker", "date"],
    cases: [
      "TC01: Fill today's date in the date input and verify the value",
      "TC02: Enter a birthday date and assert the value is stored",
      "TC03: Fill a date range - start date and end date",
      "TC04: Verify date input rejects out-of-range date (min/max constraint)",
      "TC05: Clear a date input and verify it becomes empty",
    ],
  },
  {
    slug: "links",
    label: "Links",
    path: "/practice/links",
    smokeTerms: ["links", "navigation"],
    cases: [
      "TC01: Verify link navigates to the correct URL on click",
      "TC02: Verify link text matches expected label",
      "TC03: Verify external link opens in a new tab",
      "TC04: Verify internal link stays in the same tab",
      "TC05: Verify broken link returns HTTP error status",
      "TC06: Verify link is keyboard accessible",
      "TC07: Verify link href attribute contains the correct URL",
      "TC08: Verify link has accessible label for screen readers",
      "TC09: Verify link hover state is visually distinct",
      "TC10: Verify right-click on link shows browser context menu",
      "TC11: Verify link with dynamic URL resolves correctly",
      "TC12: Verify link page loads without console errors",
    ],
  },
  {
    slug: "tabs-windows",
    label: "Tabs & Windows",
    path: "/practice/tabs-windows",
    smokeTerms: ["tabs", "windows"],
    cases: [
      "TC01: Open a link in a new tab and switch to it",
      "TC02: Open multiple windows and print all window titles",
      "TC03: Switch back to the parent window after switching to child",
      "TC04: Close the child window and verify focus returns to parent",
      "TC05: Verify Ctrl+click opens a link in a new tab",
    ],
  },
  {
    slug: "dynamic-waits",
    label: "Dynamic Waits",
    path: "/practice/dynamic-waits",
    smokeTerms: ["dynamic waits", "wait"],
    cases: [
      "TC01: Wait for a delayed browser alert to appear and accept it",
      "TC02: Wait for a hidden element to become visible after a delay",
      "TC03: Wait for a disabled button to become enabled",
      "TC04: Wait for loading text to change to a loaded state",
      "TC05: Wait for a spinner to disappear before asserting completion",
    ],
  },
  {
    slug: "multi-select",
    label: "Multi Select",
    path: "/practice/multi-select",
    smokeTerms: ["multi-select", "select"],
    cases: [
      "TC01: Select multiple fruits using Ctrl+click in a native multi-select",
      "TC02: Deselect a specific option from a pre-selected multi-select",
      "TC03: Select all countries using the Select All button",
      "TC04: Check multiple checkboxes and verify selected output",
      "TC05: Add a tag and then remove it from the chip-based multi-select",
    ],
  },
  {
    slug: "file-upload",
    label: "File Upload",
    path: "/practice/file-upload",
    smokeTerms: ["file upload", "upload"],
    cases: [
      "Upload TC01: Verify a file can be selected for upload",
      "Upload TC02: Verify selected file name is displayed after selection",
      "Upload TC06: Verify error message for unsupported file type",
      "Upload TC07: Verify error message for files exceeding size limit",
      "Upload TC08: Verify uploaded file appears in the file list",
      "Upload TC09: Verify file upload can be cancelled",
      "Upload TC10: Verify multiple files can be selected when allowed",
      "Upload TC11: Verify file input accepts only allowed extensions",
      "Upload TC12: Verify file upload is responsive on mobile viewport",
      "Upload TC13: Verify file upload is accessible via keyboard",
      "Upload TC14: Verify file upload component has accessible label",
      "Upload TC15: Verify file upload page loads without errors",
      "Download TC01: Verify download starts on clicking the download button",
      "Download TC02: Verify downloaded file name matches expected value",
      "Download TC03: Verify downloaded file is not empty",
      "Download TC04: Verify downloaded file content matches expected data",
      "Download TC05: Verify download button has accessible label",
      "Download TC06: Verify download button is keyboard accessible",
      "Download TC07: Verify download link href attribute is correct",
      "Download TC08: Verify download works on different browsers",
      "Download TC09: Verify download is responsive on mobile viewport",
      "Download TC10: Verify multiple downloads can occur sequentially",
      "Download TC11: Verify download does not navigate away from page",
      "Download TC12: Verify download section is accessible via keyboard",
      "Download TC13: Verify file download page loads without errors",
      "Download TC14: Verify download file type matches expected MIME type",
    ],
  },
];

const fileUploadUrl = "https://qaplayground.com/practice/file-upload";
const uploadValidFixture = "evals/fixtures/upload-valid.txt";
const uploadUnsupportedFixture = "evals/fixtures/upload-unsupported.exe";

function fileUploadDetailedCase(
  group: "Upload" | "Download",
  tc: string,
  title: string,
  steps: string[],
  extraConstraints: string[] = [],
): DetailedCase {
  const numberedSteps = steps.map((step, index) => `${index + 1}. ${step}`).join("\n");
  return {
    pageSlug: "file-upload",
    tc: `TC${tc}`,
    prefix: `${group.toLowerCase()}-`,
    prompt: `Use the agent-qa skill to record and replay this QA Playground File Upload test case:

${group} TC${tc}: ${title}

${numberedSteps}

Do not write Selenium or Playwright code. Use agent-qa and agent-browser only. Do not assert static tutorial or test-case documentation text as a substitute for the live upload/download component state.`,
    extraConstraints: [
      "This eval case should cover exactly one documented File Upload test case, not every upload and download behavior on the page.",
      "Treat Selenium sendKeys() and Playwright setInputFiles() mentions in the source TC as intent only; do not use Selenium, Playwright, Puppeteer, or their APIs.",
      "For file selection, drive the page with agent-browser upload and record action method uploadBySelector, for example args [\"#file-upload\", \"evals/fixtures/upload-valid.txt\"]. Do not record file selection as fillBySelector.",
      "Prefer data-testid or id selectors for the file input, upload button, download button, status messages, and file list when present.",
      ...extraConstraints,
    ],
  };
}

const fileUploadDetailedCases: DetailedCase[] = [
  fileUploadDetailedCase("Upload", "01", "Verify a file can be selected for upload", [
    `Navigate to ${fileUploadUrl}.`,
    `Locate the file input using data-testid or id, then select ${uploadValidFixture}.`,
    "Verify in the live browser that the selected file name upload-valid.txt appears in the upload field, label, or adjacent selected-file display.",
    "Record the upload action with uploadBySelector and replay the scenario successfully.",
    "If a replay-side filename assertion after upload times out, report the post-upload DOM-check framework gap instead of converting the upload action to fillBySelector.",
  ], [
    `Use the repo-relative fixture ${uploadValidFixture}; do not hard-code an absolute machine-specific file path in the prompt or scenario intent.`,
    "The current portable replay contract is the do/upload step. Verify filename display live before flushing; do not require a replay-side DOM text check if it times out after upload.",
  ]),
  fileUploadDetailedCase("Upload", "02", "Verify selected file name is displayed after selection", [
    `Navigate to ${fileUploadUrl}.`,
    `Select ${uploadValidFixture} with the file input.`,
    "Verify live that the displayed file name exactly matches upload-valid.txt.",
    "Replay the upload scenario successfully, or report the post-upload filename assertion gap if replay-side DOM checks time out.",
  ], [
    "Do not accept a broad page text match unless it comes from the live selected-file display, because the page may contain instructional text.",
    "Do not replace uploadBySelector with fillBySelector to make the filename assertion pass; that records typing, not file upload.",
  ]),
  fileUploadDetailedCase("Upload", "06", "Verify error message for unsupported file type", [
    `Navigate to ${fileUploadUrl}.`,
    `Select ${uploadUnsupportedFixture}.`,
    "Attempt to upload if the upload button becomes available.",
    "Assert a visible unsupported-file-type error appears.",
    "Replay the scenario successfully, or report that the page does not enforce unsupported extensions.",
  ], [
    `Use the repo-relative fixture ${uploadUnsupportedFixture}. It is intentionally tiny and named with an .exe extension for validation only.`,
  ]),
  fileUploadDetailedCase("Upload", "07", "Verify error message for files exceeding size limit", [
    `Navigate to ${fileUploadUrl}.`,
    "Read the page text or file input constraints to determine the stated size limit.",
    "Create a temporary oversized fixture under the eval result directory, larger than the stated limit, without committing it to the repo.",
    "Select the oversized file and attempt to upload.",
    "Assert a visible file-size error appears.",
    "Replay the scenario successfully, or report that no size limit is discoverable/enforced.",
  ], [
    "Do not add a large binary fixture to the repository. Generate oversized data only under the isolated eval results directory.",
  ]),
  fileUploadDetailedCase("Upload", "08", "Verify uploaded file appears in the file list", [
    `Navigate to ${fileUploadUrl}.`,
    `Upload ${uploadValidFixture}.`,
    "After upload succeeds, assert upload-valid.txt appears in the uploaded-file list or history section.",
    "Replay the scenario successfully.",
  ]),
  fileUploadDetailedCase("Upload", "09", "Verify file upload can be cancelled", [
    `Navigate to ${fileUploadUrl}.`,
    `Select ${uploadValidFixture} and begin upload.`,
    "Click the cancel button if one is available during upload.",
    "Assert the upload stops and the field resets or the selected-file display is cleared.",
    "Replay the scenario successfully, or report that no cancel control exists.",
  ], [
    "Do not fake cancellation by refreshing the page unless that is the documented cancel behavior on the component.",
  ]),
  fileUploadDetailedCase("Upload", "10", "Verify multiple files can be selected when allowed", [
    `Navigate to ${fileUploadUrl}.`,
    "Locate a file input that has the multiple attribute.",
    `Select ${uploadValidFixture} and ${uploadUnsupportedFixture} together only if multiple selection is allowed.`,
    "Assert both file names are displayed.",
    "Replay the scenario successfully, or report that the file input is single-file only.",
  ], [
    "Do not force multi-file selection if the input lacks the multiple attribute; report the page capability instead.",
  ]),
  fileUploadDetailedCase("Upload", "11", "Verify file input accepts only allowed extensions", [
    `Navigate to ${fileUploadUrl}.`,
    "Read the accept attribute of the file input.",
    "Assert the accept value contains only the expected allowed file types documented by the page.",
    "Replay the scenario successfully, or report if accept-attribute assertions are not record/replay supported.",
  ], [
    "Verify the accept attribute live with agent-browser before flushing; record the strongest replayable assertion available.",
  ]),
  fileUploadDetailedCase("Upload", "12", "Verify file upload is responsive on mobile viewport", [
    `Navigate to ${fileUploadUrl} at a 375px-wide mobile viewport.`,
    "Assert the upload control is fully visible and usable without horizontal scroll.",
    `Select and upload ${uploadValidFixture}.`,
    "Assert the success state is visible in the mobile viewport.",
    "Replay the scenario successfully, or report the viewport/scroll assertion gap.",
  ], [
    "Use a viewport-setting command if the current agent-browser skill exposes one; otherwise report the missing viewport capability instead of silently running desktop-only.",
  ]),
  fileUploadDetailedCase("Upload", "13", "Verify file upload is accessible via keyboard", [
    `Navigate to ${fileUploadUrl}.`,
    "Tab to the file input or its associated label/control.",
    "Press Enter or Space to activate the upload control.",
    "Assert the file dialog opens if the framework can observe native file dialogs, otherwise report the native file-dialog observability gap.",
    "Replay the scenario successfully when supported.",
  ], [
    "Native OS file dialogs are a framework boundary. Do not replace this with direct file selection unless you clearly report the keyboard-dialog gap.",
  ]),
  fileUploadDetailedCase("Upload", "14", "Verify file upload component has accessible label", [
    `Navigate to ${fileUploadUrl}.`,
    "Assert the file input has an associated label, aria-label, aria-labelledby, or equivalent accessible name.",
    "Assert a screen reader can identify the upload control from the accessible name or label relationship.",
    "Replay the scenario successfully, or report if accessibility-tree assertions are unsupported.",
  ], [
    "Prefer an ARIA/accessibility snapshot if available; otherwise verify the DOM label relationship live and record the strongest replayable selector assertion.",
  ]),
  fileUploadDetailedCase("Upload", "15", "Verify file upload page loads without errors", [
    `Navigate to ${fileUploadUrl}.`,
    "Assert the navigation succeeds and the URL remains /practice/file-upload.",
    "Assert no JavaScript console errors are present if console inspection is supported.",
    "Assert the upload input element is visible in the DOM.",
    "Replay the scenario successfully.",
  ], [
    "If HTTP status or console-error assertions are not record/replay supported, report that limitation and still record URL plus upload-input visibility checks.",
  ]),
  fileUploadDetailedCase("Download", "01", "Verify download starts on clicking the download button", [
    `Navigate to ${fileUploadUrl} and locate the download section.`,
    "Locate the download button using data-testid, id, or accessible name.",
    "Configure the eval download directory or set up a download listener if supported.",
    "Click the download button.",
    "Assert a file is saved in the expected download directory.",
    "Replay the scenario successfully, or report the download-capture support gap.",
  ], [
    "Use the isolated eval result directory for downloads when possible; do not rely on the user's default Downloads folder.",
  ]),
  fileUploadDetailedCase("Download", "02", "Verify downloaded file name matches expected value", [
    `Navigate to ${fileUploadUrl} and locate the download section.`,
    "Click the download button.",
    "Assert the downloaded file name matches the expected filename from the button/link download attribute or page text.",
    "Replay the scenario successfully, or report the download filename observability gap.",
  ]),
  fileUploadDetailedCase("Download", "03", "Verify downloaded file is not empty", [
    `Navigate to ${fileUploadUrl} and locate the download section.`,
    "Click the download button.",
    "Read the downloaded file from the isolated download directory.",
    "Assert the file size is greater than 0 bytes.",
    "Replay the scenario successfully, or report the download-file inspection gap.",
  ]),
  fileUploadDetailedCase("Download", "04", "Verify downloaded file content matches expected data", [
    `Navigate to ${fileUploadUrl} and locate the download section.`,
    "Click the download button.",
    "Read the downloaded file contents.",
    "Assert the content matches the expected data, sample text, or documented file format from the page.",
    "Replay the scenario successfully, or report the deterministic-content gap.",
  ]),
  fileUploadDetailedCase("Download", "05", "Verify download button has accessible label", [
    `Navigate to ${fileUploadUrl} and locate the download section.`,
    "Assert the download button has descriptive visible text, aria-label, or aria-labelledby.",
    "Assert a screen reader can identify the download action from the accessible name.",
    "Replay the scenario successfully, or report if accessibility-tree assertions are unsupported.",
  ]),
  fileUploadDetailedCase("Download", "06", "Verify download button is keyboard accessible", [
    `Navigate to ${fileUploadUrl} and locate the download section.`,
    "Tab to the download button.",
    "Press Enter to trigger the download.",
    "Assert the download starts or the download file appears in the isolated directory.",
    "Replay the scenario successfully, or report the keyboard/download-capture gap.",
  ]),
  fileUploadDetailedCase("Download", "07", "Verify download link href attribute is correct", [
    `Navigate to ${fileUploadUrl} and locate the download section.`,
    "Read the href and/or download attribute of the download anchor or button-backed link.",
    "Assert the URL or file path is valid and matches the expected downloadable asset for this page.",
    "Replay the scenario successfully, or report if attribute assertions are unsupported.",
  ]),
  fileUploadDetailedCase("Download", "08", "Verify download works on different browsers", [
    `Navigate to ${fileUploadUrl} in each supported browser runner available to the eval environment.`,
    "Trigger the same download in Chrome, Firefox, and Edge if those browsers are supported by agent-qa/agent-browser in this environment.",
    "Assert the file downloads successfully on each browser.",
    "Assert the downloaded file content is identical across browsers.",
    "Replay the scenario successfully, or report the cross-browser support gap.",
  ], [
    "Do not claim cross-browser coverage from a Chrome-only run. If only Chrome/CDP is supported, report that this is a framework capability gap.",
  ]),
  fileUploadDetailedCase("Download", "09", "Verify download is responsive on mobile viewport", [
    `Navigate to ${fileUploadUrl} at a 375px-wide mobile viewport and locate the download section.`,
    "Assert the download button is visible and tappable without horizontal scroll.",
    "Trigger the download and assert it initiates.",
    "Replay the scenario successfully, or report the viewport/download support gap.",
  ]),
  fileUploadDetailedCase("Download", "10", "Verify multiple downloads can occur sequentially", [
    `Navigate to ${fileUploadUrl} and locate the download section.`,
    "Click the download button twice in sequence.",
    "Assert two separate files are saved or that the same target file is deterministically overwritten according to browser behavior.",
    "Replay the scenario successfully, or report the download-capture support gap.",
  ]),
  fileUploadDetailedCase("Download", "11", "Verify download does not navigate away from page", [
    `Navigate to ${fileUploadUrl} and locate the download section.`,
    "Record the current page URL.",
    "Click the download button.",
    "Assert the page URL is still /practice/file-upload after the download triggers.",
    "Replay the scenario successfully.",
  ]),
  fileUploadDetailedCase("Download", "12", "Verify download section is accessible via keyboard", [
    `Navigate to ${fileUploadUrl}.`,
    "Tab through the page until the download section is reached.",
    "Assert the download button receives visible focus.",
    "Replay the scenario successfully, or report if focus visibility assertions are unsupported.",
  ]),
  fileUploadDetailedCase("Download", "13", "Verify file download page loads without errors", [
    `Navigate to ${fileUploadUrl}.`,
    "Assert the navigation succeeds and the URL remains /practice/file-upload.",
    "Assert no JavaScript console errors are present if console inspection is supported.",
    "Assert the download button is visible.",
    "Replay the scenario successfully.",
  ], [
    "If HTTP status or console-error assertions are not record/replay supported, report that limitation and still record URL plus download-button visibility checks.",
  ]),
  fileUploadDetailedCase("Download", "14", "Verify download file type matches expected MIME type", [
    `Navigate to ${fileUploadUrl} and locate the download section.`,
    "Intercept or inspect the download network request if supported.",
    "Assert the Content-Type header matches the expected file type for the downloadable asset.",
    "Replay the scenario successfully, or report the network-header/download interception support gap.",
  ], [
    "Do not infer MIME type only from the filename extension if response-header inspection is available.",
  ]),
];

const detailedCases: DetailedCase[] = [
  ...fileUploadDetailedCases,
  {
    pageSlug: "dropdowns",
    tc: "TC01",
    prompt: `Use the agent-qa skill to record and replay this QA Playground Dropdowns test case:

TC01: Select 'Apple' from fruit dropdown by visible text

1. Navigate to https://qaplayground.com/practice/dropdowns.
2. Locate the fruit dropdown using id dropdown-fruit or data-testid dropdown-fruit.
3. Select Apple from the fruit dropdown.
4. Verify in the live browser that Apple is the selected fruit before flushing.
5. Record the strongest replayable assertion available, such as the fruit result display containing Apple or the selected option state if supported.
6. Replay the scenario successfully.

Do not write Selenium or Playwright code. Use agent-qa and agent-browser only. Do not use the tutorial implementation text as the assertion.`,
    extraConstraints: [
      "Drive the dropdown with agent-browser select, then record action method selectBySelector with args [\"#dropdown-fruit\", \"apple\"].",
      "If the recorder cannot represent a native selected-option assertion directly, verify it live and record a replayable result/display assertion instead.",
    ],
  },
  {
    pageSlug: "dropdowns",
    tc: "TC02",
    prompt: `Use the agent-qa skill to record and replay this QA Playground Dropdowns test case:

TC02: Select 'India' from country dropdown by value attribute

1. Navigate to https://qaplayground.com/practice/dropdowns.
2. Locate the country dropdown using id dropdown-country or data-testid dropdown-country.
3. Select the option whose value is india.
4. Verify in the live browser that the selected country text is India before flushing.
5. Record the strongest replayable assertion available, such as a country result display containing India or the selected option state if supported.
6. Replay the scenario successfully.

Do not write Selenium or Playwright code. Use agent-qa and agent-browser only. Do not use the tutorial implementation text as the assertion.`,
    extraConstraints: [
      "Drive the dropdown with agent-browser select, then record action method selectBySelector with args [\"#dropdown-country\", \"india\"].",
      "The selected option value is india and the expected visible text is India.",
    ],
  },
  {
    pageSlug: "dropdowns",
    tc: "TC03",
    prompt: `Use the agent-qa skill to record and replay this QA Playground Dropdowns test case:

TC03: Verify selected value is displayed after selection

1. Navigate to https://qaplayground.com/practice/dropdowns.
2. Select Banana from the fruit dropdown using id dropdown-fruit or data-testid dropdown-fruit.
3. Locate the fruit result display using data-testid result-fruit.
4. Assert the result display text contains Banana.
5. Replay the scenario successfully.

Do not write Selenium or Playwright code. Use agent-qa and agent-browser only. Do not assert static page documentation text as a substitute for the result display.`,
    extraConstraints: [
      "Drive the dropdown with agent-browser select, then record action method selectBySelector with args [\"#dropdown-fruit\", \"banana\"].",
      "Prefer [data-testid=\"result-fruit\"] for the result check.",
      "Avoid a broad text assertion for Banana unless it is scoped or tied to the result display; the page may include option/tutorial text.",
    ],
  },
  {
    pageSlug: "dropdowns",
    tc: "TC04",
    prompt: `Use the agent-qa skill to record and replay this QA Playground Dropdowns test case:

TC04: Get all available options from the programming language dropdown

1. Navigate to https://qaplayground.com/practice/dropdowns.
2. Locate the programming language dropdown using data-testid dropdown-language.
3. Verify in the live browser that the dropdown has exactly three options.
4. Verify the option texts include Python, Java, and JavaScript.
5. Record replayable checks that prove the language dropdown loaded and remains available.
6. Replay the scenario successfully, or report the exact recorder gap if option-count/text assertions cannot be represented.

Do not write Selenium or Playwright code. Use agent-qa and agent-browser only. Do not use the tutorial implementation text as proof of the options.`,
    extraConstraints: [
      "Use selector #dropdown-language or [data-testid=\"dropdown-language\"] for live option checks.",
      "If count assertions are unsupported in record/replay, report that limitation and keep the scenario to the strongest replayable dropdown-presence checks.",
    ],
  },
  {
    pageSlug: "dropdowns",
    tc: "TC05",
    prompt: `Use the agent-qa skill to record and replay this QA Playground Dropdowns test case:

TC05: Select the last option from the programming language dropdown

1. Navigate to https://qaplayground.com/practice/dropdowns.
2. Locate the programming language dropdown using data-testid dropdown-language.
3. Select the last option, JavaScript.
4. Verify in the live browser that JavaScript is selected before flushing.
5. Record the strongest replayable assertion available, such as a language result display containing JavaScript or the selected option state if supported.
6. Replay the scenario successfully.

Do not write Selenium or Playwright code. Use agent-qa and agent-browser only. Do not use the tutorial implementation text as the assertion.`,
    extraConstraints: [
      "Drive the dropdown with agent-browser select, then record action method selectBySelector with args [\"#dropdown-language\", \"javascript\"].",
      "The expected final selection is JavaScript.",
    ],
  },
  {
    pageSlug: "dropdowns",
    tc: "TC06",
    prompt: `Use the agent-qa skill to record and replay this QA Playground Dropdowns test case:

TC06: Multi-select: select multiple superheroes

1. Navigate to https://qaplayground.com/practice/dropdowns.
2. Locate the heroes multi-select using data-testid dropdown-heroes.
3. Select Ant-Man and Batman.
4. Verify in the live browser that exactly those two heroes are selected before flushing.
5. Record the strongest replayable assertion available, such as a heroes result display containing Ant-Man and Batman or selected option state if supported.
6. Replay the scenario successfully.

Do not write Selenium or Playwright code. Use agent-qa and agent-browser only. Do not use tutorial implementation text as the assertion.`,
    extraConstraints: [
      "Drive the multi-select with agent-browser select, then record action method selectBySelector with args [\"#dropdown-heroes\", \"ant-man\"] and another selectBySelector action for \"batman\".",
      "The selected option values are ant-man and batman; the expected visible texts are Ant-Man and Batman.",
    ],
  },
  {
    pageSlug: "dropdowns",
    tc: "TC07",
    prompt: `Use the agent-qa skill to record and replay this QA Playground Dropdowns test case:

TC07: Multi-select: deselect a previously selected option

1. Navigate to https://qaplayground.com/practice/dropdowns.
2. Select Ant-Man and Aquaman from the heroes multi-select using data-testid dropdown-heroes.
3. Deselect Ant-Man so only Aquaman remains selected.
4. Verify in the live browser that Aquaman is selected and Ant-Man is not selected before flushing.
5. Record the strongest replayable assertion available, such as a heroes result display containing only Aquaman or selected option state if supported.
6. Replay the scenario successfully.

Do not write Selenium or Playwright code. Use agent-qa and agent-browser only. Do not use tutorial implementation text as the assertion.`,
    extraConstraints: [
      "Drive the multi-select with agent-browser select. Record selectBySelector actions for [\"#dropdown-heroes\", \"ant-man\"], [\"#dropdown-heroes\", \"aquaman\"], then [\"#dropdown-heroes\", \"aquaman\"] for the final selected state if deselect gestures are not replayable.",
      "If native deselect gestures are not replayable, prefer setting the multi-select to only aquaman and report the limitation clearly.",
    ],
  },
  {
    pageSlug: "dropdowns",
    tc: "TC08",
    prompt: `Use the agent-qa skill to record and replay this QA Playground Dropdowns test case:

TC08: Verify default placeholder text before any selection

1. Navigate to https://qaplayground.com/practice/dropdowns.
2. Locate the fruit dropdown before interacting with it using data-testid dropdown-fruit.
3. Verify the default placeholder or selected option text reads Select Fruit.
4. Record a replayable assertion that proves the initial fruit dropdown state is present.
5. Replay the scenario successfully.

Do not write Selenium or Playwright code. Use agent-qa and agent-browser only. Do not select any fruit for this test.`,
    extraConstraints: [
      "Use #dropdown-fruit or [data-testid=\"dropdown-fruit\"] for the initial state check.",
      "Avoid broad page-text assertions for Select Fruit unless scoped to the dropdown; docs or examples can duplicate that text.",
    ],
  },
  {
    pageSlug: "dropdowns",
    tc: "TC09",
    prompt: `Use the agent-qa skill to record and replay this QA Playground Dropdowns test case:

TC09: Verify a dropdown is enabled and interactable

1. Navigate to https://qaplayground.com/practice/dropdowns.
2. Locate the country dropdown using data-testid dropdown-country.
3. Verify in the live browser that the dropdown is enabled.
4. Select a country option and verify the selection succeeds without error.
5. Record the strongest replayable assertion available for the successful country selection.
6. Replay the scenario successfully.

Do not write Selenium or Playwright code. Use agent-qa and agent-browser only. Do not use tutorial implementation text as the assertion.`,
    extraConstraints: [
      "Drive the dropdown with agent-browser select, then record action method selectBySelector with args [\"#dropdown-country\", \"india\"] or another valid country value.",
      "If enabled-state assertions are unsupported in record/replay, verify enabled state live and record the successful selection path as replay evidence.",
    ],
  },
  {
    pageSlug: "dropdowns",
    tc: "TC10",
    prompt: `Use the agent-qa skill to record and replay this QA Playground Dropdowns test case:

TC10: Verify the total count of options in the country dropdown

1. Navigate to https://qaplayground.com/practice/dropdowns.
2. Locate the country dropdown using data-testid dropdown-country.
3. Verify in the live browser that the country dropdown has exactly four options.
4. Verify the option texts are India, USA, UK, and Argentina.
5. Record replayable checks that prove the country dropdown loaded and remains available.
6. Replay the scenario successfully, or report the exact recorder gap if option-count/text assertions cannot be represented.

Do not write Selenium or Playwright code. Use agent-qa and agent-browser only. Do not use the tutorial implementation text as proof of the options.`,
    extraConstraints: [
      "Use selector #dropdown-country or [data-testid=\"dropdown-country\"] for live option checks.",
      "If count assertions are unsupported in record/replay, report that limitation and keep the scenario to the strongest replayable dropdown-presence checks.",
    ],
  },
  {
    pageSlug: "forms",
    tc: "TC01",
    prompt: `Use the agent-qa skill to record and replay this QA Playground Forms test case:

TC01: Fill all fields with valid data and submit successfully

1. Navigate to https://qaplayground.com/practice/forms.
2. Fill input-first-name with 'John'.
3. Fill input-last-name with 'Doe'.
4. Fill input-email with 'john@example.com'.
5. Fill input-phone with '9876543210'.
6. Fill input-dob with a valid date.
7. Select radio-gender-male.
8. Select 'India' from select-country dropdown.
9. Fill input-city with 'Mumbai'.
10. Fill input-password with 'pass123'.
11. Fill input-confirm-password with 'pass123'.
12. Check checkbox-terms.
13. Click submit-form-btn.
14. Assert the success state is visible by checking visible success text such as "Form submitted successfully" or submitted-name containing "John".

Do not record an assert where the first arg is a CSS selector like [data-testid="form-success-msg"]. The current recorder assert shape accepts role/name pairs or URL assertions only.`,
    extraConstraints: [
      "Prefer the named data-testid values from the steps when accessible names are ambiguous.",
      "Fixed form values are literals. Do not use fill-unique.",
      "For the final success assertion, use a role/name or visible text assertion that replay can resolve; do not encode a CSS selector as an assert role.",
    ],
  },
  {
    pageSlug: "alerts-dialogs",
    tc: "TC01",
    prompt: `Use the agent-qa skill to record and replay this QA Playground Alerts & Dialogs test case:

TC01: Accept a simple browser alert and verify it closes

1. Navigate to https://qaplayground.com/practice/alerts-dialogs.
2. Click the Simple Alert button using data-testid btn-simple-alert.
3. Accept the native browser alert.
4. Assert the alert is dismissed and the page remains interactive.
5. Replay the scenario successfully, or report the exact native-dialog framework gap if agent-qa cannot represent/replay alert acceptance.

Do not write Selenium or Playwright code. Use agent-qa and agent-browser only. Do not fake success by only clicking tutorial text or by asserting static page documentation.`,
    extraConstraints: [
      "Native browser alert acceptance is a framework boundary. If replay cannot represent it, report that gap instead of replacing the test with a DOM-only assertion.",
      "The page contains Selenium/Playwright tutorial text and test-case text; do not use that static documentation as proof that the alert was handled.",
    ],
  },
  {
    pageSlug: "alerts-dialogs",
    tc: "TC02",
    prompt: `Use the agent-qa skill to record and replay this QA Playground Alerts & Dialogs test case:

TC02: Get text from a simple browser alert before accepting

1. Navigate to https://qaplayground.com/practice/alerts-dialogs.
2. Click the Simple Alert button using data-testid btn-simple-alert.
3. Read the native alert message and verify it is Welcome to QA PlayGround!.
4. Accept the alert after reading the text.
5. Replay the scenario successfully, or report the exact native-dialog framework gap if agent-qa cannot represent/replay alert text reads.

Do not write Selenium or Playwright code. Use agent-qa and agent-browser only. Do not assert static documentation text as a substitute for the browser alert message.`,
    extraConstraints: [
      "This case requires observing native alert text. If agent-qa lacks a read/assert dialog step, report the framework gap clearly.",
      "Do not record a broad text assertion for Welcome to QA PlayGround! unless it comes from the actual dialog handling path.",
    ],
  },
  {
    pageSlug: "alerts-dialogs",
    tc: "TC03",
    prompt: `Use the agent-qa skill to record and replay this QA Playground Alerts & Dialogs test case:

TC03: Accept a confirm dialog and verify accepted state

1. Navigate to https://qaplayground.com/practice/alerts-dialogs.
2. Click the Confirm Alert button using data-testid btn-confirm-alert.
3. Accept the native confirm dialog.
4. Assert the result display shows Accepted.
5. Replay the scenario successfully, or report the exact native-dialog framework gap if agent-qa cannot represent/replay confirm acceptance.

Do not write Selenium or Playwright code. Use agent-qa and agent-browser only. Do not use static test-case documentation text as the result assertion.`,
    extraConstraints: [
      "Native confirm acceptance is a framework boundary. If unsupported, report the gap instead of faking the Accepted state.",
      "Prefer selector-scoped or live DOM checks for the post-confirm result if dialog handling succeeds.",
    ],
  },
  {
    pageSlug: "alerts-dialogs",
    tc: "TC04",
    prompt: `Use the agent-qa skill to record and replay this QA Playground Alerts & Dialogs test case:

TC04: Dismiss a confirm dialog and verify dismissed state

1. Navigate to https://qaplayground.com/practice/alerts-dialogs.
2. Click the Confirm Alert button using data-testid btn-confirm-alert.
3. Dismiss the native confirm dialog.
4. Assert the result display shows Dismissed.
5. Replay the scenario successfully, or report the exact native-dialog framework gap if agent-qa cannot represent/replay confirm dismissal.

Do not write Selenium or Playwright code. Use agent-qa and agent-browser only. Do not use static test-case documentation text as the result assertion.`,
    extraConstraints: [
      "Native confirm dismissal is a framework boundary. If unsupported, report the gap instead of faking the Dismissed state.",
      "Prefer selector-scoped or live DOM checks for the post-confirm result if dialog handling succeeds.",
    ],
  },
  {
    pageSlug: "alerts-dialogs",
    tc: "TC05",
    prompt: `Use the agent-qa skill to record and replay this QA Playground Alerts & Dialogs test case:

TC05: Enter text in a prompt dialog and accept it

1. Navigate to https://qaplayground.com/practice/alerts-dialogs.
2. Click the Prompt Alert button using data-testid btn-prompt-alert.
3. Type John Doe into the native prompt dialog and accept it.
4. Assert the prompt result display shows Your name is - John Doe.
5. Replay the scenario successfully, or report the exact native-dialog framework gap if agent-qa cannot represent/replay prompt input.

Do not write Selenium or Playwright code. Use agent-qa and agent-browser only. Do not fake the result by directly editing the DOM.`,
    extraConstraints: [
      "Native prompt input is a framework boundary. If unsupported, report the gap instead of replacing the prompt with a normal DOM fill.",
      "John Doe is a fixed literal for this test. Do not use fill-unique.",
    ],
  },
  {
    pageSlug: "alerts-dialogs",
    tc: "TC06",
    prompt: `Use the agent-qa skill to record and replay this QA Playground Alerts & Dialogs test case:

TC06: Dismiss a prompt dialog and verify no input is captured

1. Navigate to https://qaplayground.com/practice/alerts-dialogs.
2. Click the Prompt Alert button using data-testid btn-prompt-alert.
3. Dismiss the native prompt dialog without entering text.
4. Assert the prompt result display is empty or not visible.
5. Replay the scenario successfully, or report the exact native-dialog framework gap if agent-qa cannot represent/replay prompt dismissal.

Do not write Selenium or Playwright code. Use agent-qa and agent-browser only. Do not use static page documentation as the assertion.`,
    extraConstraints: [
      "Native prompt dismissal is a framework boundary. If unsupported, report the gap instead of faking an empty result.",
      "Prefer selector absence or scoped DOM text checks for the prompt result if dialog handling succeeds.",
    ],
  },
  {
    pageSlug: "alerts-dialogs",
    tc: "TC07",
    prompt: `Use the agent-qa skill to record and replay this QA Playground Alerts & Dialogs test case:

TC07: Verify toast notification appears after triggering

1. Navigate to https://qaplayground.com/practice/alerts-dialogs.
2. Click the Toast Alert button using data-testid btn-toast-alert.
3. Immediately verify in the live browser that [data-sonner-toast] text contains This is simple toast.
4. Record a replayable wait for selector [data-sonner-toast].
5. Replay the scenario successfully.

Do not write Selenium or Playwright code. Use agent-qa and agent-browser only. Do not assert the tutorial text that mentions toast notifications.`,
    extraConstraints: [
      "The toast is transient. A separate replay selectorText step can miss it after selector presence succeeds; verify text live before flushing and keep replay assertion to selector presence unless the framework gains atomic selector+text waiting.",
      "Do not use a broad text wait for This is simple toast because page documentation can contain similar text.",
    ],
  },
  {
    pageSlug: "alerts-dialogs",
    tc: "TC08",
    prompt: `Use the agent-qa skill to record and replay this QA Playground Alerts & Dialogs test case:

TC08: Close a modal/sweet alert using the Cancel button

1. Navigate to https://qaplayground.com/practice/alerts-dialogs.
2. Click the Sweet Alert button using data-testid btn-modal-alert.
3. Wait for the alertdialog containing Modern Alert to appear.
4. Click the You Are! cancel button using data-testid btn-modal-cancel.
5. Assert the alertdialog is no longer visible.
6. Replay the scenario successfully.

Do not write Selenium or Playwright code. Use agent-qa and agent-browser only. Do not use driver.switchTo().alert(); this is a custom DOM modal, not a native browser alert.`,
    extraConstraints: [
      "Use selector [role=\"alertdialog\"] for modal presence and absence; the stable cancel button is [data-testid=\"btn-modal-cancel\"].",
      "Avoid broad text assertions for Modern Alert because the docs can mention modal/sweet alert behavior.",
    ],
  },
  {
    pageSlug: "alerts-dialogs",
    tc: "TC09",
    prompt: `Use the agent-qa skill to record and replay this QA Playground Alerts & Dialogs test case:

TC09: Close an advanced dialog using the Close button

1. Navigate to https://qaplayground.com/practice/alerts-dialogs.
2. Click the Share button using data-testid btn-dialog-share.
3. Wait for the dialog to open and assert [data-testid=\"input-share-link\"] is visible.
4. Assert the input value contains qaplayground.com/practice/alerts-dialogs.
5. Click the Close button using data-testid btn-dialog-close.
6. Assert the dialog is dismissed.
7. Replay the scenario successfully.

Do not write Selenium or Playwright code. Use agent-qa and agent-browser only. Do not assert static page documentation text as a substitute for the share dialog state.`,
    extraConstraints: [
      "The share dialog uses role=dialog, input [data-testid=\"input-share-link\"], and close button [data-testid=\"btn-dialog-close\"].",
      "Because recorder assertions do not support input value assertions directly, verify the input value with a live DOM check before flushing and record selector presence/absence waits for replay.",
    ],
  },
  {
    pageSlug: "alerts-dialogs",
    tc: "TC10",
    prompt: `Use the agent-qa skill to record and replay this QA Playground Alerts & Dialogs test case:

TC10: Verify alerts page loads without errors

1. Navigate to https://qaplayground.com/practice/alerts-dialogs.
2. Assert the page URL is /practice/alerts-dialogs.
3. Assert the page content includes Alerts & Dialogs Automation Practice.
4. Assert all six trigger buttons are visible: btn-simple-alert, btn-confirm-alert, btn-prompt-alert, btn-toast-alert, btn-modal-alert, and btn-dialog-share.
5. Replay the scenario successfully.

Do not write Selenium or Playwright code. Use agent-qa and agent-browser only.`,
    extraConstraints: [
      "Use selector waits for the six trigger buttons rather than broad text checks, because the page also contains documentation and test-case text.",
      "If checking console errors or HTTP status is unsupported by record/replay, report that limitation and still record the strongest replayable page-load checks.",
    ],
  },
  {
    pageSlug: "dynamic-waits",
    tc: "TC01",
    prompt: `Use the agent-qa skill to record and replay this QA Playground Dynamic Waits test case:

TC01: Wait for a delayed browser alert to appear and accept it

1. Navigate to https://qaplayground.com/practice/dynamic-waits.
2. Click the Trigger Delayed Alert button using data-testid btn-delayed-alert or visible text.
3. Record an agent-qa wait long enough for the delayed alert to fire.
4. Assert the page remains responsive after the delayed alert path, proving the scenario did not timeout.
5. Replay the scenario successfully.

Do not write Selenium or Playwright code. Use agent-qa record-step actions and waits only. Do not hand-edit generated scenario artifacts.`,
    extraConstraints: [
      "This page contains Selenium/Playwright tutorial text; do not use those instructions as the scenario body.",
      "Native dialog handling is a framework boundary. If replay cannot represent the alert accept flow, report the framework gap instead of faking success.",
    ],
  },
  {
    pageSlug: "dynamic-waits",
    tc: "TC02",
    prompt: `Use the agent-qa skill to record and replay this QA Playground Dynamic Waits test case:

TC02: Wait for a hidden element to become visible after a delay

1. Navigate to https://qaplayground.com/practice/dynamic-waits.
2. Click the Show Element button using data-testid btn-show-element.
3. Record a wait for selector [data-testid="delayed-element"].
4. Assert that the live delayed element text is Element is now visible! before flushing.
5. Replay the scenario successfully.

Do not write Selenium or Playwright code. Use agent-qa record-step actions and waits only. Do not hand-edit generated scenario artifacts.`,
    extraConstraints: [
      "Prefer selector waits over broad text waits because the page includes tutorial and test-case documentation text.",
    ],
  },
  {
    pageSlug: "dynamic-waits",
    tc: "TC03",
    prompt: `Use the agent-qa skill to record and replay this QA Playground Dynamic Waits test case:

TC03: Wait for a disabled button to become enabled

1. Navigate to https://qaplayground.com/practice/dynamic-waits.
2. Verify the target button [data-testid="btn-enable-after-delay"] is initially disabled with a selector/state check.
3. Click the Activate Button control by visible text.
4. Record a wait for selector [data-testid="btn-enable-after-delay"]:not([disabled]).
5. Replay the scenario successfully.

Do not write Selenium or Playwright code. Use agent-qa record-step actions and waits only. Do not hand-edit generated scenario artifacts.`,
    extraConstraints: [
      "Live inspection showed raw selector click did not start the countdown, but visible-text click did. Prefer clickByText for Activate Button.",
    ],
  },
  {
    pageSlug: "dynamic-waits",
    tc: "TC04",
    prompt: `Use the agent-qa skill to record and replay this QA Playground Dynamic Waits test case:

TC04: Wait for loading text to change to a loaded state

1. Navigate to https://qaplayground.com/practice/dynamic-waits.
2. Click the Load Data button using data-testid btn-load-data or visible text.
3. Record a selectorText wait scoped to main content for Data Loaded!.
4. Assert the live page reaches Data Loaded! before flushing.
5. Replay the scenario successfully.

Do not write Selenium or Playwright code. Use agent-qa record-step actions and waits only. Do not hand-edit generated scenario artifacts.`,
    extraConstraints: [
      "Do not use a broad text wait for Data Loaded! unless it is scoped; the docs also mention that text.",
    ],
  },
  {
    pageSlug: "dynamic-waits",
    tc: "TC05",
    prompt: `Use the agent-qa skill to record and replay this QA Playground Dynamic Waits test case:

TC05: Wait for a spinner to disappear before asserting completion

1. Navigate to https://qaplayground.com/practice/dynamic-waits.
2. Click the Start Spinner button using data-testid btn-start-spinner or visible text.
3. Record a wait that proves the spinner is gone or the done state is visible.
4. Assert the live page reaches the completion state before flushing.
5. Replay the scenario successfully.

Do not write Selenium or Playwright code. Use agent-qa record-step actions and waits only. Do not hand-edit generated scenario artifacts.`,
    extraConstraints: [
      "Prefer selector-scoped or live DOM checks over broad text waits because this page includes docs text about spinners.",
    ],
  },
];

function tcNumber(title: string): string {
  const match = title.match(/TC(?:-[A-Z]+)?-?(\d+)/i);
  return match?.[1] || "00";
}

function tcPrefix(title: string): string {
  const named = title.match(/^TC-([A-Z]+)-\d+/i)?.[1]?.toLowerCase();
  if (named) return `${named}-`;
  if (/^Upload\s+TC/i.test(title)) return "upload-";
  if (/^Download\s+TC/i.test(title)) return "download-";
  return "";
}

function slugify(value: string): string {
  return value
    .toLowerCase()
    .replace(/&/g, "and")
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-|-$/g, "")
    .slice(0, 80);
}

function pageUrl(page: PracticePage): string {
  return `https://qaplayground.com${page.path}`;
}

function detailedCaseFor(page: PracticePage, title: string): DetailedCase | undefined {
  const tc = `TC${tcNumber(title).padStart(2, "0")}`;
  const prefix = tcPrefix(title);
  return detailedCases.find((item) =>
    item.pageSlug === page.slug &&
    item.tc === tc &&
    (item.prefix === undefined || item.prefix === prefix)
  );
}

function qaplaygroundCase(page: PracticePage, title: string): EvalCase {
  const url = pageUrl(page);
  const tc = `${tcPrefix(title)}tc${tcNumber(title).padStart(2, "0")}`;
  const detail = detailedCaseFor(page, title);
  const cleanTitle = title.replace(/^((Upload|Download)\s+)?TC(?:-[A-Z]+)?-?\d+:\s*/i, "");
  return {
    id: `qaplayground-${page.slug}-${tc}-${slugify(cleanTitle)}`,
    suite: "qaplayground",
    page: page.slug,
    name: `QA Playground ${page.label} ${title}`,
    prompt: detail?.prompt || `Use the agent-qa skill to record and replay this QA Playground test case:

${title}

1. Navigate to ${url}.
2. Perform the user interactions needed for this test case.
3. Assert the expected result for this test case.
4. Replay the scenario successfully.`,
    extraConstraints: [
      "This eval case should cover exactly one documented QA Playground test case, not the entire page.",
      "Prefer stable data-testid-backed selectors when the test case names them or when accessible names are ambiguous.",
      ...(detail?.extraConstraints || []),
    ],
    scenarioMatches(scenario: unknown): boolean {
      const text = textOf(scenario);
      return text.includes(url.toLowerCase()) &&
        cleanTitle
          .toLowerCase()
          .split(/\s+/)
          .filter((word) => word.length >= 4)
          .slice(0, 4)
          .some((word) => text.includes(word));
    },
  };
}

function qaplaygroundPageCase(page: PracticePage): EvalCase {
  const url = pageUrl(page);
  return {
    id: `qaplayground-${page.slug}-page-load`,
    suite: "qaplayground",
    page: page.slug,
    name: `QA Playground ${page.label} page loads`,
    prompt: `Use the agent-qa skill to record and replay this QA Playground page-load smoke test:

1. Navigate to ${url}.
2. Verify the page loads without errors.
3. Verify the page content includes ${page.smokeTerms.map((item) => `"${item}"`).join(" and ")}.
4. Replay the scenario successfully.`,
    extraConstraints: [
      "Keep this as a page-smoke scenario. Do not interact with every widget on the page.",
      "Use a URL or present-content assertion that proves the intended practice page loaded.",
    ],
    scenarioMatches(scenario: unknown): boolean {
      const text = textOf(scenario);
      return text.includes(url.toLowerCase()) &&
        page.smokeTerms.some((term) => text.includes(term));
    },
  };
}

const qaplaygroundCases = practicePages.flatMap((page) => [
  qaplaygroundPageCase(page),
  ...page.cases.map((title) => qaplaygroundCase(page, title)),
]);

const automationExerciseUrl = "https://www.automationexercise.com";
const automationExerciseUploadFixture = "evals/fixtures/upload-valid.txt";

const automationExerciseSelectorHints = [
  "Known stable Automation Exercise selectors: Signup/Login link a[href=\"/login\"], Products link a[href=\"/products\"], Cart link a[href=\"/view_cart\"], Contact Us link a[href=\"/contact_us\"], Test Cases link a[href=\"/test_cases\"].",
  "Known login selectors: input[data-qa=\"login-email\"], input[data-qa=\"login-password\"], button[data-qa=\"login-button\"].",
  "Known signup selectors: input[data-qa=\"signup-name\"], input[data-qa=\"signup-email\"], button[data-qa=\"signup-button\"].",
  "Known account form selectors: input[data-qa=\"password\"], select[data-qa=\"days\"], select[data-qa=\"months\"], select[data-qa=\"years\"], input[data-qa=\"first_name\"], input[data-qa=\"last_name\"], input[data-qa=\"company\"], input[data-qa=\"address\"], input[data-qa=\"address2\"], select[data-qa=\"country\"], input[data-qa=\"state\"], input[data-qa=\"city\"], input[data-qa=\"zipcode\"], input[data-qa=\"mobile_number\"], button[data-qa=\"create-account\"].",
  "Known contact selectors: input[data-qa=\"name\"], input[data-qa=\"email\"], input[data-qa=\"subject\"], textarea[data-qa=\"message\"], input[name=\"upload_file\"], input[data-qa=\"submit-button\"].",
  "Known product/search selectors: #search_product, #submit_search, a[href^=\"/product_details/\"], a[data-product-id].",
  "Use agent-browser eval to click a CSS selector only when agent-browser has no direct selector verb for that action; then record the corresponding record-step action method such as clickSelector.",
];

const disposableAccountConstraints = [
  "Use a unique disposable email address for each run, for example agent-qa+<timestamp>@example.com, so replay is not coupled to previous site state.",
  "If a documented flow needs a correct login and no seeded credentials are provided, create a disposable account as setup in the same scenario, then run the documented login steps with that account.",
  "When the documented case deletes the account, finish by verifying ACCOUNT DELETED! and do not reuse that account in later evals.",
];

const automationExerciseGoldenConstraints: Record<string, string[]> = {
  TC01: [
    "Golden flow: open home, then navigate directly to https://www.automationexercise.com/login; do not snapshot the home page to discover Signup/Login.",
    "Use signup/account selectors input[data-qa=\"signup-name\"], input[data-qa=\"signup-email\"], button[data-qa=\"signup-button\"], #id_gender1, input[data-qa=\"password\"], select[data-qa=\"days\"], select[data-qa=\"months\"], select[data-qa=\"years\"], #newsletter, #optin, address data-qa fields, and button[data-qa=\"create-account\"].",
    "Record waits/assertions for input[data-qa=\"signup-email\"], ENTER ACCOUNT INFORMATION, h2[data-qa=\"account-created\"] containing Account Created!, Logged in as the unique name, and h2[data-qa=\"account-deleted\"] containing Account Deleted!.",
  ],
  TC02: [
    "Golden flow: create a disposable account as setup on /login, log out with a[href=\"/logout\"], then perform the documented login with that same email/password and delete the account.",
    "Use login selectors input[data-qa=\"login-email\"], input[data-qa=\"login-password\"], and button[data-qa=\"login-button\"]. Record /login after logout, Logged in as the unique name, and Account Deleted!.",
  ],
  TC03: [
    "Golden flow: open home, then navigate directly to https://www.automationexercise.com/login; do not snapshot the home page or click through navigation for this case.",
    "Use intentionally invalid unique-looking credentials, wait for input[data-qa=\"login-email\"], fill the two login data-qa fields, click button[data-qa=\"login-button\"], then record a visible text wait for Your email or password is incorrect!.",
  ],
  TC04: [
    "Golden flow: start directly at https://www.automationexercise.com/login, create a disposable account, verify Logged in as the unique name, click a[href=\"/logout\"], assert /login and Login to your account, then log back in only for cleanup deletion.",
    "Do not stop after logout if a disposable account was created; clean it up with login data-qa fields, a[href=\"/delete_account\"], and h2[data-qa=\"account-deleted\"].",
  ],
  TC05: [
    "Golden flow: create a disposable account on /login, log out, attempt signup again with the same email, verify Email Address already exist!, then log in and delete the account for cleanup.",
    "Use signup/login data-qa selectors and a[href=\"/logout\"]/a[href=\"/delete_account\"]; do not use a real or pre-existing email address.",
  ],
  TC06: [
    "Golden boundary: record the filled contact form, upload selection, and submit-button availability only. Do not click Submit or try to accept the native confirm in replay; it is not replay-stable in this harness.",
    `Use route https://www.automationexercise.com/contact_us via Contact Us link or direct navigation, fill input[data-qa=\"name\"], input[data-qa=\"email\"], input[data-qa=\"subject\"], textarea[data-qa=\"message\"], and upload ${automationExerciseUploadFixture} with input[name=\"upload_file\"] as uploadBySelector.`,
    "Record waits for /contact_us, GET IN TOUCH, and input[data-qa=\"submit-button\"] presence/availability. Verify the selected filename live before flushing if needed.",
  ],
  TC07: [
    "Golden flow: open home, click selector a[href=\"/test_cases\"] using agent-browser eval or smart-click, record clickSelector, then assert URL /test_cases and record a text wait for Test Cases. Do not invent selectorText on main; this page does not guarantee a main element.",
    "Do not record present/absent asserts with CSS selectors encoded as roles. For the page content check, use record-step wait text Test Cases or selectorText on a stable selector instead.",
  ],
  TC08: [
    "Golden flow: after home navigation, navigate directly to https://www.automationexercise.com/products; do not snapshot the huge products page unless selector checks fail.",
    "Wait for and assert ALL PRODUCTS plus a[href^=\"/product_details/\"], click the first product detail link, then assert /product_details/ URL and visible detail markers Category:, Availability:, Condition:, and Brand:.",
  ],
  TC09: [
    "Golden flow: after home navigation, navigate directly to https://www.automationexercise.com/products; do not click through or snapshot the product catalog.",
    "Minimal command sequence after start: open home and record navigation; open /products and record navigation; record wait selector #search_product; fill #search_product with jeans and record fillBySelector; click #submit_search and record clickSelector; record wait text SEARCHED PRODUCTS; record wait selectorText .features_items Soft Stretch Jeans; flush, verify, replay.",
    "Do not add extra snapshots, product-list discovery, option counting, or duplicate URL assertions for this case.",
  ],
  TC10: [
    "Golden flow: stay on home, scroll #footer into view with agent-browser eval, fill #susbscribe_email with a unique disposable subscription email, and click #subscribe.",
    "Use footer selectors #footer, #footer .single-widget h2, #susbscribe_email, #subscribe, and #success-subscribe; the input id is misspelled susbscribe on the site.",
    "Verify #success-subscribe:not(.hide) and the exact success text live before flushing, but record replay-stable #success-subscribe if visible-state replay is flaky.",
  ],
  TC11: [
    "Golden flow: click a[href=\"/view_cart\"], assert /view_cart and #cart_items .breadcrumb containing Shopping Cart, then perform the footer subscription flow.",
    "Reuse TC10 selectors and caveat: #footer, #susbscribe_email, #subscribe, #success-subscribe, live-only #success-subscribe:not(.hide), and a unique disposable subscription email.",
  ],
  TC12: [
    "Golden flow: navigate directly to https://www.automationexercise.com/products, add product ids 1 and 2 using .features_items .productinfo a.add-to-cart[data-product-id=\"1\"] and [data-product-id=\"2\"].",
    "After each add, record a wait for #cartModal.show; use #cartModal.show .close-modal for Continue Shopping and #cartModal.show a[href=\"/view_cart\"] for View Cart.",
    "After clicking Continue Shopping, verify the first modal has closed before adding product 2; after adding product 2, verify #cartModal.show a[href=\"/view_cart\"] is live-visible before clicking it.",
    "Verify #product-1 and #product-2 rows with Blue Top/Men Tshirt, prices Rs. 500/Rs. 400, quantities 1, and matching totals.",
  ],
  TC13: [
    "Golden flow: use product 1 for determinism, either click .features_items .choose a[href=\"/product_details/1\"] from home or navigate to https://www.automationexercise.com/product_details/1 after home.",
    "Set #quantity to 4, record fillBySelector [\"#quantity\", \"4\"], click .product-information button.cart, wait for #cartModal.show, open #cartModal.show a[href=\"/view_cart\"], and verify #product-1 quantity 4 plus total Rs. 2000.",
  ],
  TC14: [
    "Golden flow: add Blue Top before registration, open cart, click .check_out while logged out, use #checkoutModal.show a[href=\"/login\"] to register during checkout, then return to cart and checkout.",
    "Use product selector .features_items .productinfo a.add-to-cart[data-product-id=\"1\"], modal selectors #cartModal.show and #checkoutModal.show, checkout selectors .check_out, #address_delivery, #cart_info, #ordermsg textarea[name=\"message\"], and a[href=\"/payment\"].check_out.",
    "Use payment selectors input[data-qa=\"name-on-card\"], input[data-qa=\"card-number\"], input[data-qa=\"cvc\"], input[data-qa=\"expiry-month\"], input[data-qa=\"expiry-year\"], and button[data-qa=\"pay-button\"]. Delete the disposable account after order success.",
  ],
  TC15: [
    "Golden flow: create the disposable account first on /login, then navigate directly to /products, add Blue Top, checkout, pay, and delete the account.",
    "Use the same account, product, checkout, payment, and delete selectors as TC14; assert #address_delivery contains Agent QA and #cart_info contains Blue Top before payment.",
  ],
  TC16: [
    "Golden flow: create a disposable account as setup on /login, log out, log back in with input[data-qa=\"login-email\"] and input[data-qa=\"login-password\"], then add Blue Top, checkout, pay, and delete the account.",
    "Use the same product, checkout, payment, and delete selectors as TC14/TC15. Do not assume public seeded credentials exist.",
  ],
  TC17: [
    "Golden flow: navigate directly to /products, add product 1 Blue Top, wait #cartModal.show, open cart, then remove with #product-1 a.cart_quantity_delete.",
    "Assert #empty_cart and visible text Cart is empty! after removal rather than relying only on absence of product text.",
  ],
  TC18: [
    "Golden flow: use the category sidebar accordion, click #accordian a[href=\"#Women\"], then #Women a[href=\"/category_products/2\"] for Women - Tops Products; then click #accordian a[href=\"#Men\"] and #Men a[href=\"/category_products/3\"] for Men - Tshirts Products.",
    "Record selectorText waits using DOM text case Women - Tops Products and Men - Tshirts Products. The source steps mention Women > Dress but expect Tops; choose Tops to satisfy the expected heading and report that source mismatch if needed.",
  ],
  TC19: [
    "Golden flow: navigate directly to /products, assert .brands_products, click a[href=\"/brand_products/Polo\"], verify Brand - Polo Products and product detail links, then click a[href=\"/brand_products/H&M\"] and verify Brand - H&M Products.",
    "Use exact href selectors for brands; H&M contains an ampersand in the href and should not be URL-escaped in the CSS selector.",
  ],
  TC20: [
    "Golden flow: create a disposable account first on /login, stay logged in, navigate directly to /products, search jeans, add searched product id 33, verify cart, then delete the account.",
    "Use #search_product, #submit_search, .features_items text Soft Stretch Jeans, .features_items .productinfo a.add-to-cart[data-product-id=\"33\"], #cartModal.show a[href=\"/view_cart\"], #cart_info, and a[href=\"/delete_account\"].",
  ],
  TC21: [
    "Golden flow: navigate directly to /products, click a[href^=\"/product_details/\"] for product 1, wait for Write Your Review, fill #name, #email, and #review, click #button-review, then assert Thank you for your review.",
    "Use disposable review data and a unique email address. Do not submit broad page feedback or tutorial text as the review assertion.",
  ],
  TC22: [
    "Golden flow: scroll .recommended_items into view with agent-browser eval, assert RECOMMENDED ITEMS, click .recommended_items a.add-to-cart[data-product-id], wait #cartModal.show, open #cartModal.show a[href=\"/view_cart\"], then assert a tr[id^=\"product-\"] exists in cart.",
  ],
  TC23: [
    "Golden flow: create a disposable account on /login using address 123 Example Street, navigate directly to /products, add Blue Top, open /view_cart directly after the modal, checkout, verify delivery and billing addresses, then delete the account.",
    "Assert #address_delivery and #address_invoice contain the registered address. Use #product-1 and #cart_items .breadcrumb to prove the cart before checkout.",
  ],
  TC24: [
    "Golden flow: register during checkout like TC14, pay successfully, verify the order-success page and invoice link a[href^=\"/download_invoice/\"], click it, continue, then delete the account.",
    "Download boundary: click the invoice link and report the download-capture gap if the framework cannot inspect the saved file. Do not depend on the user's default Downloads folder.",
  ],
  TC25: [
    "Golden flow: scroll #footer into view with agent-browser eval, assert SUBSCRIPTION, click #scrollUp, then assert the top hero text Full-Fledged practice website for Automation Engineers is visible.",
  ],
  TC26: [
    "Golden flow: scroll #footer into view with agent-browser eval, assert SUBSCRIPTION, then use a replayable scroll action or agent-browser eval window.scrollTo(0, 0) and record the corresponding scroll intent; assert the top hero text Full-Fledged practice website for Automation Engineers is visible.",
  ],
};

function automationExerciseCase(testCase: AutomationExerciseTestCase): EvalCase {
  const numberedSteps = testCase.steps.map((step, index) => `${index + 1}. ${step}`).join("\n");
  const idSuffix = slugify(testCase.title);
  return {
    id: `automation-exercise-${testCase.tc.toLowerCase()}-${idSuffix}`,
    suite: "automation-exercise",
    page: "automationexercise.com",
    name: `Automation Exercise ${testCase.tc}: ${testCase.title}`,
    prompt: `Use the agent-qa skill to record and replay this Automation Exercise test case:

${testCase.tc}: ${testCase.title}

${numberedSteps}

Do not write Selenium or Playwright code. Use agent-qa and agent-browser only. Record the strongest replayable assertions for the visible page state, URL, cart state, account state, or success message named by the test case.`,
    extraConstraints: [
      "This eval case should cover exactly one documented Automation Exercise test case, not the full site catalog.",
      "Start from the public site only. Do not bootstrap private profiles or vendor-specific state.",
      "Prefer stable selectors such as data-qa attributes, form field names, visible link/button names, and route URLs over brittle generated CSS selectors.",
      "Do not assert static test-case documentation text as a substitute for the live page state after interacting with the site.",
      "For account, checkout, subscription, contact, review, and cart mutations, use disposable eval data and avoid real personal/payment information.",
      "Avoid broad snapshots on Automation Exercise home/products/search/category pages. Use direct routes and selector waits from this prompt before interacting with large pages.",
      "Do not run agent-browser launch, clickSelector, clickRole, fillBySelector, selectBySelector, uploadBySelector, wait-for-selector, waitForSelector, or wait as browser verbs. Those method names belong in agent-qa record-step payloads; waits are agent-qa record-step wait payloads.",
      ...automationExerciseSelectorHints,
      ...(automationExerciseGoldenConstraints[testCase.tc] || []),
      ...(testCase.extraConstraints || []),
    ],
    fastPath: testCase.fastPath,
    scenarioMatches(scenario: unknown): boolean {
      const text = textOf(scenario);
      const titleWords = testCase.title
        .toLowerCase()
        .split(/\s+/)
        .filter((word) => word.length >= 4)
        .slice(0, 5);
      return text.includes("automationexercise.com") &&
        titleWords.some((word) => text.includes(word));
    },
  };
}

const automationExerciseTestCases: AutomationExerciseTestCase[] = [
  {
    tc: "TC01",
    title: "Register User",
    steps: [
      "Launch browser.",
      `Navigate to ${automationExerciseUrl}.`,
      "Verify that home page is visible successfully.",
      "Click on Signup / Login button.",
      "Verify New User Signup! is visible.",
      "Enter a disposable name and unique email address.",
      "Click Signup button.",
      "Verify that ENTER ACCOUNT INFORMATION is visible.",
      "Fill details: Title, Name, Email, Password, Date of birth.",
      "Select checkbox Sign up for our newsletter!.",
      "Select checkbox Receive special offers from our partners!.",
      "Fill details: First name, Last name, Company, Address, Address2, Country, State, City, Zipcode, Mobile Number.",
      "Click Create Account button.",
      "Verify that ACCOUNT CREATED! is visible.",
      "Click Continue button.",
      "Verify that Logged in as username is visible.",
      "Click Delete Account button.",
      "Verify that ACCOUNT DELETED! is visible and click Continue button.",
    ],
    extraConstraints: disposableAccountConstraints,
  },
  {
    tc: "TC02",
    title: "Login User with correct email and password",
    steps: [
      "Launch browser.",
      `Navigate to ${automationExerciseUrl}.`,
      "Verify that home page is visible successfully.",
      "Click on Signup / Login button.",
      "Verify Login to your account is visible.",
      "Enter correct email address and password.",
      "Click login button.",
      "Verify that Logged in as username is visible.",
      "Click Delete Account button.",
      "Verify that ACCOUNT DELETED! is visible.",
    ],
    extraConstraints: disposableAccountConstraints,
  },
  {
    tc: "TC03",
    title: "Login User with incorrect email and password",
    steps: [
      "Launch browser.",
      `Navigate to ${automationExerciseUrl}.`,
      "Verify that home page is visible successfully.",
      "Click on Signup / Login button.",
      "Verify Login to your account is visible.",
      "Enter incorrect email address and password.",
      "Click login button.",
      "Verify error Your email or password is incorrect! is visible.",
    ],
    extraConstraints: [
      "Use credentials that are intentionally invalid and unique-looking, not a real user's email or password.",
      "Do not snapshot the home page to discover Signup / Login. After recording the home-page navigation, navigate directly to https://www.automationexercise.com/login with agent-browser open and record a navigation step for that route; do not click a[href=\"/login\"] for this eval.",
      "Before filling, record and satisfy a wait for selector input[data-qa=\"login-email\"] or verify it live with agent-browser eval. Do not fill immediately after route change without waiting for the selector.",
      "Use agent-browser fill with quoted CSS selectors for input[data-qa=\"login-email\"] and input[data-qa=\"login-password\"], then record fillBySelector actions for those same selectors.",
      "Use agent-browser click with quoted CSS selector button[data-qa=\"login-button\"], then record clickSelector for button[data-qa=\"login-button\"].",
      "Record the final check as a wait/assert for visible text Your email or password is incorrect!, then flush, verify, and replay. Do not add extra page discovery after the login form is visible.",
    ],
    fastPath: [
      `AGENT_QA_SCENARIOS_DIR={scenarios} AGENT_QA_RECORD_DIR={record} {qa} start "TC03 invalid login" --session {session}`,
      `{browser} --session {session} open ${automationExerciseUrl}`,
      `AGENT_QA_SCENARIOS_DIR={scenarios} AGENT_QA_RECORD_DIR={record} {qa} record-step navigation '{"route":"${automationExerciseUrl}/"}'`,
      `{browser} --session {session} open ${automationExerciseUrl}/login`,
      `AGENT_QA_SCENARIOS_DIR={scenarios} AGENT_QA_RECORD_DIR={record} {qa} record-step navigation '{"route":"${automationExerciseUrl}/login"}'`,
      `AGENT_QA_SCENARIOS_DIR={scenarios} AGENT_QA_RECORD_DIR={record} {qa} record-step wait '{"condition":{"kind":"selector","selector":"input[data-qa=\\"login-email\\"]"},"intent":"login email field visible"}'`,
      `{browser} --session {session} fill 'input[data-qa="login-email"]' invalid-agent-qa@example.com`,
      `AGENT_QA_SCENARIOS_DIR={scenarios} AGENT_QA_RECORD_DIR={record} {qa} record-step action '{"method":"fillBySelector","args":["input[data-qa=\\"login-email\\"]","invalid-agent-qa@example.com"],"intent":"enter invalid email"}'`,
      `{browser} --session {session} fill 'input[data-qa="login-password"]' not-the-password`,
      `AGENT_QA_SCENARIOS_DIR={scenarios} AGENT_QA_RECORD_DIR={record} {qa} record-step action '{"method":"fillBySelector","args":["input[data-qa=\\"login-password\\"]","not-the-password"],"intent":"enter invalid password"}'`,
      `{browser} --session {session} click 'button[data-qa="login-button"]'`,
      `AGENT_QA_SCENARIOS_DIR={scenarios} AGENT_QA_RECORD_DIR={record} {qa} record-step action '{"method":"clickSelector","args":["button[data-qa=\\"login-button\\"]"],"intent":"submit invalid login"}'`,
      `AGENT_QA_SCENARIOS_DIR={scenarios} AGENT_QA_RECORD_DIR={record} {qa} record-step wait '{"condition":{"kind":"text","text":"Your email or password is incorrect!"},"intent":"invalid login error visible"}'`,
      `AGENT_QA_SCENARIOS_DIR={scenarios} AGENT_QA_RECORD_DIR={record} {qa} flush`,
      `AGENT_QA_SCENARIOS_DIR={scenarios} AGENT_QA_RECORD_DIR={record} {qa} verify`,
      `AGENT_QA_SCENARIOS_DIR={scenarios} AGENT_QA_RECORD_DIR={record} {qa} replay <sid> --session {session}-replay`,
    ],
  },
  {
    tc: "TC04",
    title: "Logout User",
    steps: [
      "Launch browser.",
      `Navigate to ${automationExerciseUrl}.`,
      "Verify that home page is visible successfully.",
      "Click on Signup / Login button.",
      "Verify Login to your account is visible.",
      "Enter correct email address and password.",
      "Click login button.",
      "Verify that Logged in as username is visible.",
      "Click Logout button.",
      "Verify that user is navigated to login page.",
    ],
    extraConstraints: disposableAccountConstraints,
  },
  {
    tc: "TC05",
    title: "Register User with existing email",
    steps: [
      "Launch browser.",
      `Navigate to ${automationExerciseUrl}.`,
      "Verify that home page is visible successfully.",
      "Click on Signup / Login button.",
      "Verify New User Signup! is visible.",
      "Enter name and already registered email address.",
      "Click Signup button.",
      "Verify error Email Address already exist! is visible.",
    ],
    extraConstraints: [
      ...disposableAccountConstraints,
      "Create the disposable account as setup if needed, log out, then attempt signup again with the same email to trigger the existing-email error.",
      "After verifying the existing-email error, log in with the disposable account and delete it so replay can recreate the same account.",
    ],
  },
  {
    tc: "TC06",
    title: "Contact Us Form",
    steps: [
      "Launch browser.",
      `Navigate to ${automationExerciseUrl}.`,
      "Verify that home page is visible successfully.",
      "Click on Contact Us button.",
      "Verify GET IN TOUCH is visible.",
      "Enter name, email, subject and message.",
      `Upload file ${automationExerciseUploadFixture}.`,
      "Click Submit button.",
      "Click OK button on the confirmation dialog.",
      "Verify success message Success! Your details have been submitted successfully. is visible.",
      "Click Home button and verify that landed to home page successfully.",
    ],
    extraConstraints: [
      `Use the repo-relative fixture ${automationExerciseUploadFixture}; do not hard-code an absolute machine-specific path.`,
      "Native confirmation dialog handling is a framework boundary. If replay cannot represent OK/accept, report that exact gap instead of faking success.",
      "Known deterministic golden records the filled contact form and upload selection, then stops before the native confirm because replayable native-confirm acceptance is not stable in this harness.",
    ],
  },
  {
    tc: "TC07",
    title: "Verify Test Cases Page",
    steps: [
      "Launch browser.",
      `Navigate to ${automationExerciseUrl}.`,
      "Verify that home page is visible successfully.",
      "Click on Test Cases button.",
      "Verify user is navigated to test cases page successfully.",
    ],
    extraConstraints: [
      "Use agent-qa smart-click \"Test Cases\" or drive a DOM click on selector a[href=\"/test_cases\"] if visible-text click is ambiguous because the navbar includes icon glyphs.",
      "Do not run agent-browser clickSelector; clickSelector is only a record-step action method after the browser action has already been driven.",
      "A known-good deterministic recording uses navigation to https://www.automationexercise.com, clickSelector a[href=\"/test_cases\"], URL assert /test_cases, and visible text Test Cases.",
    ],
    fastPath: [
      `AGENT_QA_SCENARIOS_DIR={scenarios} AGENT_QA_RECORD_DIR={record} {qa} start "TC07 test cases page" --session {session}`,
      `{browser} --session {session} open ${automationExerciseUrl}`,
      `AGENT_QA_SCENARIOS_DIR={scenarios} AGENT_QA_RECORD_DIR={record} {qa} record-step navigation '{"route":"${automationExerciseUrl}"}'`,
      `{browser} --session {session} click 'a[href="/test_cases"]'`,
      `AGENT_QA_SCENARIOS_DIR={scenarios} AGENT_QA_RECORD_DIR={record} {qa} record-step action '{"method":"clickSelector","args":["a[href=\\"/test_cases\\"]"],"intent":"open test cases page"}'`,
      `AGENT_QA_SCENARIOS_DIR={scenarios} AGENT_QA_RECORD_DIR={record} {qa} record-step assert '{"kind":"url","args":["/test_cases"],"intent":"test cases URL reached"}'`,
      `AGENT_QA_SCENARIOS_DIR={scenarios} AGENT_QA_RECORD_DIR={record} {qa} record-step wait '{"condition":{"kind":"text","text":"Test Cases"},"intent":"test cases page visible"}'`,
      `AGENT_QA_SCENARIOS_DIR={scenarios} AGENT_QA_RECORD_DIR={record} {qa} flush`,
      `AGENT_QA_SCENARIOS_DIR={scenarios} AGENT_QA_RECORD_DIR={record} {qa} verify`,
      `AGENT_QA_SCENARIOS_DIR={scenarios} AGENT_QA_RECORD_DIR={record} {qa} replay <sid> --session {session}-replay`,
    ],
  },
  {
    tc: "TC08",
    title: "Verify All Products and product detail page",
    steps: [
      "Launch browser.",
      `Navigate to ${automationExerciseUrl}.`,
      "Verify that home page is visible successfully.",
      "Click on Products button.",
      "Verify user is navigated to ALL PRODUCTS page successfully.",
      "Verify the products list is visible.",
      "Click on View Product of first product.",
      "Verify user is landed to product detail page.",
      "Verify that product detail is visible: product name, category, price, availability, condition, brand.",
    ],
    extraConstraints: [
      "Do not run agent-browser launch; opening the URL launches the browser session.",
      "Do not snapshot the full home/products page unless a named selector fails; the products page is large and snapshots are expensive.",
      "After recording the home-page navigation, navigate directly to https://www.automationexercise.com/products with agent-browser open and record a navigation step for that route; do not click a[href=\"/products\"] for this eval.",
      "Before interacting with products, record and satisfy a wait for selector a[href^=\"/product_details/\"]. Do not snapshot the full products page unless that selector is missing.",
      "Assert /products URL, visible text ALL PRODUCTS, and presence of a[href^=\"/product_details/\"].",
      "Click the first product detail link with selector a[href^=\"/product_details/\"], record clickSelector, then assert /product_details/ URL plus visible texts Category:, Availability:, Condition:, and Brand:.",
    ],
    fastPath: [
      `AGENT_QA_SCENARIOS_DIR={scenarios} AGENT_QA_RECORD_DIR={record} {qa} start "TC08 products detail" --session {session}`,
      `{browser} --session {session} open ${automationExerciseUrl}`,
      `AGENT_QA_SCENARIOS_DIR={scenarios} AGENT_QA_RECORD_DIR={record} {qa} record-step navigation '{"route":"${automationExerciseUrl}"}'`,
      `{browser} --session {session} open ${automationExerciseUrl}/products`,
      `AGENT_QA_SCENARIOS_DIR={scenarios} AGENT_QA_RECORD_DIR={record} {qa} record-step navigation '{"route":"${automationExerciseUrl}/products"}'`,
      `AGENT_QA_SCENARIOS_DIR={scenarios} AGENT_QA_RECORD_DIR={record} {qa} record-step assert '{"kind":"url","args":["/products"],"intent":"products URL reached"}'`,
      `AGENT_QA_SCENARIOS_DIR={scenarios} AGENT_QA_RECORD_DIR={record} {qa} record-step wait '{"condition":{"kind":"text","text":"ALL PRODUCTS"},"intent":"all products visible"}'`,
      `AGENT_QA_SCENARIOS_DIR={scenarios} AGENT_QA_RECORD_DIR={record} {qa} record-step wait '{"condition":{"kind":"selector","selector":"a[href^=\\"/product_details/\\"]"},"intent":"product detail links visible"}'`,
      `{browser} --session {session} click 'a[href^="/product_details/"]'`,
      `AGENT_QA_SCENARIOS_DIR={scenarios} AGENT_QA_RECORD_DIR={record} {qa} record-step action '{"method":"clickSelector","args":["a[href^=\\"/product_details/\\"]"],"intent":"open product detail"}'`,
      `AGENT_QA_SCENARIOS_DIR={scenarios} AGENT_QA_RECORD_DIR={record} {qa} record-step assert '{"kind":"url","args":["/product_details/"],"intent":"product detail URL reached"}'`,
      `AGENT_QA_SCENARIOS_DIR={scenarios} AGENT_QA_RECORD_DIR={record} {qa} record-step wait '{"condition":{"kind":"text","text":"Availability:"},"intent":"availability visible"}'`,
      `AGENT_QA_SCENARIOS_DIR={scenarios} AGENT_QA_RECORD_DIR={record} {qa} record-step wait '{"condition":{"kind":"text","text":"Condition:"},"intent":"condition visible"}'`,
      `AGENT_QA_SCENARIOS_DIR={scenarios} AGENT_QA_RECORD_DIR={record} {qa} record-step wait '{"condition":{"kind":"text","text":"Brand:"},"intent":"brand visible"}'`,
      `AGENT_QA_SCENARIOS_DIR={scenarios} AGENT_QA_RECORD_DIR={record} {qa} flush`,
      `AGENT_QA_SCENARIOS_DIR={scenarios} AGENT_QA_RECORD_DIR={record} {qa} verify`,
      `AGENT_QA_SCENARIOS_DIR={scenarios} AGENT_QA_RECORD_DIR={record} {qa} replay <sid> --session {session}-replay`,
    ],
  },
  {
    tc: "TC09",
    title: "Search Product",
    steps: [
      "Launch browser.",
      `Navigate to ${automationExerciseUrl}.`,
      "Verify that home page is visible successfully.",
      "Click on Products button.",
      "Verify user is navigated to ALL PRODUCTS page successfully.",
      "Enter product name in search input and click search button.",
      "Verify SEARCHED PRODUCTS is visible.",
      "Verify all the products related to search are visible.",
    ],
    extraConstraints: [
      "Use a product keyword visible on the product list, such as dress, top, tshirt, or jeans, and assert the searched-products section rather than site documentation.",
      "Prefer the keyword jeans for deterministic results.",
      "After recording the home-page navigation, navigate directly to https://www.automationexercise.com/products with agent-browser open and record a navigation step for that route.",
      "Wait for #search_product before filling. Fill #search_product with jeans, record fillBySelector, click #submit_search, and record clickSelector.",
      "Assert SEARCHED PRODUCTS is visible and that .features_items contains Soft Stretch Jeans or another visible jeans product.",
    ],
    fastPath: [
      `AGENT_QA_SCENARIOS_DIR={scenarios} AGENT_QA_RECORD_DIR={record} {qa} start "TC09 search product" --session {session}`,
      `{browser} --session {session} open ${automationExerciseUrl}`,
      `AGENT_QA_SCENARIOS_DIR={scenarios} AGENT_QA_RECORD_DIR={record} {qa} record-step navigation '{"route":"${automationExerciseUrl}/"}'`,
      `{browser} --session {session} open ${automationExerciseUrl}/products`,
      `AGENT_QA_SCENARIOS_DIR={scenarios} AGENT_QA_RECORD_DIR={record} {qa} record-step navigation '{"route":"${automationExerciseUrl}/products"}'`,
      `AGENT_QA_SCENARIOS_DIR={scenarios} AGENT_QA_RECORD_DIR={record} {qa} record-step wait '{"condition":{"kind":"selector","selector":"#search_product"},"intent":"search input visible"}'`,
      `{browser} --session {session} fill '#search_product' jeans`,
      `AGENT_QA_SCENARIOS_DIR={scenarios} AGENT_QA_RECORD_DIR={record} {qa} record-step action '{"method":"fillBySelector","args":["#search_product","jeans"],"intent":"enter jeans search"}'`,
      `{browser} --session {session} click '#submit_search'`,
      `AGENT_QA_SCENARIOS_DIR={scenarios} AGENT_QA_RECORD_DIR={record} {qa} record-step action '{"method":"clickSelector","args":["#submit_search"],"intent":"submit product search"}'`,
      `AGENT_QA_SCENARIOS_DIR={scenarios} AGENT_QA_RECORD_DIR={record} {qa} record-step wait '{"condition":{"kind":"text","text":"SEARCHED PRODUCTS"},"intent":"searched products visible"}'`,
      `AGENT_QA_SCENARIOS_DIR={scenarios} AGENT_QA_RECORD_DIR={record} {qa} record-step wait '{"condition":{"kind":"selectorText","selector":".features_items","text":"Soft Stretch Jeans"},"intent":"jeans result visible"}'`,
      `AGENT_QA_SCENARIOS_DIR={scenarios} AGENT_QA_RECORD_DIR={record} {qa} flush`,
      `AGENT_QA_SCENARIOS_DIR={scenarios} AGENT_QA_RECORD_DIR={record} {qa} verify`,
      `AGENT_QA_SCENARIOS_DIR={scenarios} AGENT_QA_RECORD_DIR={record} {qa} replay <sid> --session {session}-replay`,
    ],
  },
  {
    tc: "TC10",
    title: "Verify Subscription in home page",
    steps: [
      "Launch browser.",
      `Navigate to ${automationExerciseUrl}.`,
      "Verify that home page is visible successfully.",
      "Scroll down to footer.",
      "Verify text SUBSCRIPTION.",
      "Enter email address in input and click arrow button.",
      "Verify success message You have been successfully subscribed! is visible.",
    ],
    extraConstraints: [
      "Use a unique disposable subscription email for each run.",
      "Use footer selectors #footer, #footer .single-widget h2, #susbscribe_email, #subscribe, and #success-subscribe. The email input id is intentionally misspelled susbscribe on the site.",
      "Do not assert only global text You have been successfully subscribed!, because the success container can exist hidden before submit. Require #success-subscribe:not(.hide) plus selectorText on #success-subscribe.",
      "Known replay caveat: the visible success state can be server/timing-sensitive on replay. Verify #success-subscribe:not(.hide) and exact success text live before flushing, but record the replay-stable selector wait #success-subscribe if the visible-state wait is flaky.",
      "Use agent-browser eval to scroll #footer into view, fill #susbscribe_email, and click #subscribe; then record scroll/fill/click actions and selector waits.",
    ],
    fastPath: [
      `AGENT_QA_SCENARIOS_DIR={scenarios} AGENT_QA_RECORD_DIR={record} {qa} start "TC10 home subscription" --session {session}`,
      `{browser} --session {session} open ${automationExerciseUrl}`,
      `AGENT_QA_SCENARIOS_DIR={scenarios} AGENT_QA_RECORD_DIR={record} {qa} record-step navigation '{"route":"${automationExerciseUrl}"}'`,
      `AGENT_QA_SCENARIOS_DIR={scenarios} AGENT_QA_RECORD_DIR={record} {qa} record-step wait '{"condition":{"kind":"selectorText","selector":"#footer .single-widget h2","text":"Subscription"},"intent":"subscription heading visible"}'`,
      `{browser} --session {session} fill '#susbscribe_email' agent-qa-subscribe@example.com`,
      `AGENT_QA_SCENARIOS_DIR={scenarios} AGENT_QA_RECORD_DIR={record} {qa} record-step action '{"method":"fillBySelector","args":["#susbscribe_email","agent-qa-subscribe@example.com"],"intent":"enter subscription email"}'`,
      `{browser} --session {session} click '#subscribe'`,
      `AGENT_QA_SCENARIOS_DIR={scenarios} AGENT_QA_RECORD_DIR={record} {qa} record-step action '{"method":"clickSelector","args":["#subscribe"],"intent":"submit subscription"}'`,
      `AGENT_QA_SCENARIOS_DIR={scenarios} AGENT_QA_RECORD_DIR={record} {qa} record-step wait '{"condition":{"kind":"selector","selector":"#success-subscribe"},"intent":"subscription success container exists"}'`,
      `AGENT_QA_SCENARIOS_DIR={scenarios} AGENT_QA_RECORD_DIR={record} {qa} flush`,
      `AGENT_QA_SCENARIOS_DIR={scenarios} AGENT_QA_RECORD_DIR={record} {qa} verify`,
      `AGENT_QA_SCENARIOS_DIR={scenarios} AGENT_QA_RECORD_DIR={record} {qa} replay <sid> --session {session}-replay`,
    ],
  },
  {
    tc: "TC11",
    title: "Verify Subscription in Cart page",
    steps: [
      "Launch browser.",
      `Navigate to ${automationExerciseUrl}.`,
      "Verify that home page is visible successfully.",
      "Click Cart button.",
      "Scroll down to footer.",
      "Verify text SUBSCRIPTION.",
      "Enter email address in input and click arrow button.",
      "Verify success message You have been successfully subscribed! is visible.",
    ],
    extraConstraints: [
      "Use a unique disposable subscription email for each run.",
      "Use Cart selector a[href=\"/view_cart\"] and verify URL /view_cart plus #cart_items .breadcrumb containing Shopping Cart before footer subscription actions.",
      "Reuse the TC10 subscription selectors and caveats: #footer, #footer .single-widget h2, #susbscribe_email, #subscribe, #success-subscribe, and live-only #success-subscribe:not(.hide).",
      "Verify the exact success message live after submit, then record replay-stable wait #success-subscribe if visible-state replay is flaky.",
    ],
    fastPath: [
      `AGENT_QA_SCENARIOS_DIR={scenarios} AGENT_QA_RECORD_DIR={record} {qa} start "TC11 cart subscription" --session {session}`,
      `{browser} --session {session} open ${automationExerciseUrl}`,
      `AGENT_QA_SCENARIOS_DIR={scenarios} AGENT_QA_RECORD_DIR={record} {qa} record-step navigation '{"route":"${automationExerciseUrl}"}'`,
      `{browser} --session {session} click 'a[href="/view_cart"]'`,
      `AGENT_QA_SCENARIOS_DIR={scenarios} AGENT_QA_RECORD_DIR={record} {qa} record-step action '{"method":"clickSelector","args":["a[href=\\"/view_cart\\"]"],"intent":"open cart page"}'`,
      `AGENT_QA_SCENARIOS_DIR={scenarios} AGENT_QA_RECORD_DIR={record} {qa} record-step assert '{"kind":"url","args":["/view_cart"],"intent":"cart URL reached"}'`,
      `AGENT_QA_SCENARIOS_DIR={scenarios} AGENT_QA_RECORD_DIR={record} {qa} record-step wait '{"condition":{"kind":"selectorText","selector":"#cart_items .breadcrumb","text":"Shopping Cart"},"intent":"cart breadcrumb visible"}'`,
      `AGENT_QA_SCENARIOS_DIR={scenarios} AGENT_QA_RECORD_DIR={record} {qa} record-step wait '{"condition":{"kind":"selectorText","selector":"#footer .single-widget h2","text":"Subscription"},"intent":"subscription heading visible"}'`,
      `{browser} --session {session} fill '#susbscribe_email' agent-qa-cart-subscribe@example.com`,
      `AGENT_QA_SCENARIOS_DIR={scenarios} AGENT_QA_RECORD_DIR={record} {qa} record-step action '{"method":"fillBySelector","args":["#susbscribe_email","agent-qa-cart-subscribe@example.com"],"intent":"enter subscription email"}'`,
      `{browser} --session {session} click '#subscribe'`,
      `AGENT_QA_SCENARIOS_DIR={scenarios} AGENT_QA_RECORD_DIR={record} {qa} record-step action '{"method":"clickSelector","args":["#subscribe"],"intent":"submit subscription"}'`,
      `AGENT_QA_SCENARIOS_DIR={scenarios} AGENT_QA_RECORD_DIR={record} {qa} record-step wait '{"condition":{"kind":"selector","selector":"#success-subscribe"},"intent":"subscription success container exists"}'`,
      `AGENT_QA_SCENARIOS_DIR={scenarios} AGENT_QA_RECORD_DIR={record} {qa} flush`,
      `AGENT_QA_SCENARIOS_DIR={scenarios} AGENT_QA_RECORD_DIR={record} {qa} verify`,
      `AGENT_QA_SCENARIOS_DIR={scenarios} AGENT_QA_RECORD_DIR={record} {qa} replay <sid> --session {session}-replay`,
    ],
  },
  {
    tc: "TC12",
    title: "Add Products in Cart",
    steps: [
      "Launch browser.",
      `Navigate to ${automationExerciseUrl}.`,
      "Verify that home page is visible successfully.",
      "Click Products button.",
      "Hover over first product and click Add to cart.",
      "Click Continue Shopping button.",
      "Hover over second product and click Add to cart.",
      "Click View Cart button.",
      "Verify both products are added to Cart.",
      "Verify their prices, quantity and total price.",
    ],
    extraConstraints: [
      "If hover-specific recording is unsupported, use the visible Add to cart buttons exposed for the first two product cards and report the hover capability gap.",
      "Use direct navigation to https://www.automationexercise.com/products after the home-page navigation.",
      "Use visible non-overlay selectors .features_items .productinfo a.add-to-cart[data-product-id=\"1\"] and [data-product-id=\"2\"] to avoid hover dependence.",
      "Wait for #cartModal.show after each add; use #cartModal.show .close-modal for Continue Shopping and #cartModal.show a[href=\"/view_cart\"] for View Cart.",
      "Verify cart rows #product-1 and #product-2 with names Blue Top and Men Tshirt, prices Rs. 500/Rs. 400, quantities 1, and matching totals.",
    ],
  },
  {
    tc: "TC13",
    title: "Verify Product quantity in Cart",
    steps: [
      "Launch browser.",
      `Navigate to ${automationExerciseUrl}.`,
      "Verify that home page is visible successfully.",
      "Click View Product for any product on home page.",
      "Verify product detail is opened.",
      "Increase quantity to 4.",
      "Click Add to cart button.",
      "Click View Cart button.",
      "Verify that product is displayed in cart page with exact quantity.",
    ],
    extraConstraints: [
      "Use product 1 for determinism: .features_items .choose a[href=\"/product_details/1\"], expected name Blue Top, price Rs. 500, cart row #product-1.",
      "Set #quantity to 4 with agent-browser eval/fill and record fillBySelector [\"#quantity\", \"4\"].",
      "Use .product-information button.cart to add to cart, #cartModal.show a[href=\"/view_cart\"] to open cart, and verify #product-1 quantity text 4 plus total Rs. 2000.",
      "Run in a fresh browser session/cart; existing cart state can make quantity assertions nondeterministic.",
    ],
  },
  {
    tc: "TC14",
    title: "Place Order: Register while Checkout",
    steps: [
      "Launch browser.",
      `Navigate to ${automationExerciseUrl}.`,
      "Verify that home page is visible successfully.",
      "Add products to cart.",
      "Click Cart button.",
      "Verify that cart page is displayed.",
      "Click Proceed To Checkout.",
      "Click Register / Login button.",
      "Fill all details in Signup and create account.",
      "Verify ACCOUNT CREATED! and click Continue button.",
      "Verify Logged in as username at top.",
      "Click Cart button.",
      "Click Proceed To Checkout button.",
      "Verify Address Details and Review Your Order.",
      "Enter description in comment text area and click Place Order.",
      "Enter payment details: Name on Card, Card Number, CVC, Expiration date.",
      "Click Pay and Confirm Order button.",
      "Verify success message Your order has been placed successfully!.",
      "Click Delete Account button.",
      "Verify ACCOUNT DELETED! and click Continue button.",
    ],
    extraConstraints: disposableAccountConstraints,
  },
  {
    tc: "TC15",
    title: "Place Order: Register before Checkout",
    steps: [
      "Launch browser.",
      `Navigate to ${automationExerciseUrl}.`,
      "Verify that home page is visible successfully.",
      "Click Signup / Login button.",
      "Fill all details in Signup and create account.",
      "Verify ACCOUNT CREATED! and click Continue button.",
      "Verify Logged in as username at top.",
      "Add products to cart.",
      "Click Cart button.",
      "Verify that cart page is displayed.",
      "Click Proceed To Checkout.",
      "Verify Address Details and Review Your Order.",
      "Enter description in comment text area and click Place Order.",
      "Enter payment details: Name on Card, Card Number, CVC, Expiration date.",
      "Click Pay and Confirm Order button.",
      "Verify success message Your order has been placed successfully!.",
      "Click Delete Account button.",
      "Verify ACCOUNT DELETED! and click Continue button.",
    ],
    extraConstraints: [
      ...disposableAccountConstraints,
      "Use the proven TC14 account/payment selectors, but create the account before adding products to cart.",
      "Use product 1 Blue Top and checkout selectors .check_out, #address_delivery, #cart_info, #ordermsg textarea[name=\"message\"], a[href=\"/payment\"].check_out, and payment data-qa fields.",
    ],
  },
  {
    tc: "TC16",
    title: "Place Order: Login before Checkout",
    steps: [
      "Launch browser.",
      `Navigate to ${automationExerciseUrl}.`,
      "Verify that home page is visible successfully.",
      "Click Signup / Login button.",
      "Fill email, password and click Login button.",
      "Verify Logged in as username at top.",
      "Add products to cart.",
      "Click Cart button.",
      "Verify that cart page is displayed.",
      "Click Proceed To Checkout.",
      "Verify Address Details and Review Your Order.",
      "Enter description in comment text area and click Place Order.",
      "Enter payment details: Name on Card, Card Number, CVC, Expiration date.",
      "Click Pay and Confirm Order button.",
      "Verify success message Your order has been placed successfully!.",
      "Click Delete Account button.",
      "Verify ACCOUNT DELETED! and click Continue button.",
    ],
    extraConstraints: [
      ...disposableAccountConstraints,
      "Because the public site has no seeded credentials, create a disposable account as setup in the same scenario, log out, then perform the documented login-before-checkout flow with that account.",
      "Use the proven TC14/TC15 account, login, cart, checkout, payment, and delete selectors.",
    ],
  },
  {
    tc: "TC17",
    title: "Remove Products From Cart",
    steps: [
      "Launch browser.",
      `Navigate to ${automationExerciseUrl}.`,
      "Verify that home page is visible successfully.",
      "Add products to cart.",
      "Click Cart button.",
      "Verify that cart page is displayed.",
      "Click X button corresponding to particular product.",
      "Verify that product is removed from the cart.",
    ],
    extraConstraints: [
      "Use product 1 Blue Top and remove it with #product-1 a.cart_quantity_delete.",
      "After removal, assert #empty_cart and visible text Cart is empty! rather than only absence of product text.",
    ],
    fastPath: [
      `AGENT_QA_SCENARIOS_DIR={scenarios} AGENT_QA_RECORD_DIR={record} {qa} start "TC17 remove cart product" --session {session}`,
      `{browser} --session {session} open ${automationExerciseUrl}`,
      `AGENT_QA_SCENARIOS_DIR={scenarios} AGENT_QA_RECORD_DIR={record} {qa} record-step navigation '{"route":"${automationExerciseUrl}"}'`,
      `{browser} --session {session} open ${automationExerciseUrl}/products`,
      `AGENT_QA_SCENARIOS_DIR={scenarios} AGENT_QA_RECORD_DIR={record} {qa} record-step navigation '{"route":"${automationExerciseUrl}/products"}'`,
      `{browser} --session {session} eval '(() => { const el = document.querySelector(".features_items .productinfo a.add-to-cart[data-product-id=\\"1\\"]"); if (!el) throw new Error("add button missing"); el.dispatchEvent(new MouseEvent("mousedown", { bubbles: true, cancelable: true, view: window })); el.dispatchEvent(new MouseEvent("mouseup", { bubbles: true, cancelable: true, view: window })); el.click(); return true; })()'`,
      `AGENT_QA_SCENARIOS_DIR={scenarios} AGENT_QA_RECORD_DIR={record} {qa} record-step action '{"method":"clickSelector","args":[".features_items .productinfo a.add-to-cart[data-product-id=\\"1\\"]"],"intent":"add Blue Top to cart"}'`,
      `AGENT_QA_SCENARIOS_DIR={scenarios} AGENT_QA_RECORD_DIR={record} {qa} record-step wait '{"condition":{"kind":"selector","selector":"#cartModal.show"},"intent":"cart modal visible"}'`,
      `{browser} --session {session} open ${automationExerciseUrl}/view_cart`,
      `AGENT_QA_SCENARIOS_DIR={scenarios} AGENT_QA_RECORD_DIR={record} {qa} record-step navigation '{"route":"${automationExerciseUrl}/view_cart"}'`,
      `AGENT_QA_SCENARIOS_DIR={scenarios} AGENT_QA_RECORD_DIR={record} {qa} record-step assert '{"kind":"url","args":["/view_cart"],"intent":"cart URL reached"}'`,
      `AGENT_QA_SCENARIOS_DIR={scenarios} AGENT_QA_RECORD_DIR={record} {qa} record-step wait '{"condition":{"kind":"selectorText","selector":"#cart_items .breadcrumb","text":"Shopping Cart"},"intent":"cart page visible"}'`,
      `AGENT_QA_SCENARIOS_DIR={scenarios} AGENT_QA_RECORD_DIR={record} {qa} record-step wait '{"condition":{"kind":"selector","selector":"#product-1"},"intent":"product row present before removal"}'`,
      `{browser} --session {session} eval '(() => { const el = document.querySelector("#product-1 a.cart_quantity_delete"); if (!el) throw new Error("delete button missing"); el.dispatchEvent(new MouseEvent("mousedown", { bubbles: true, cancelable: true, view: window })); el.dispatchEvent(new MouseEvent("mouseup", { bubbles: true, cancelable: true, view: window })); el.click(); return true; })()'`,
      `AGENT_QA_SCENARIOS_DIR={scenarios} AGENT_QA_RECORD_DIR={record} {qa} record-step action '{"method":"clickSelector","args":["#product-1 a.cart_quantity_delete"],"intent":"remove product from cart"}'`,
      `AGENT_QA_SCENARIOS_DIR={scenarios} AGENT_QA_RECORD_DIR={record} {qa} record-step wait '{"condition":{"kind":"selector","selector":"#empty_cart"},"intent":"empty cart state visible"}'`,
      `AGENT_QA_SCENARIOS_DIR={scenarios} AGENT_QA_RECORD_DIR={record} {qa} record-step wait '{"condition":{"kind":"text","text":"Cart is empty!"},"intent":"cart empty message visible"}'`,
      `AGENT_QA_SCENARIOS_DIR={scenarios} AGENT_QA_RECORD_DIR={record} {qa} flush`,
      `AGENT_QA_SCENARIOS_DIR={scenarios} AGENT_QA_RECORD_DIR={record} {qa} verify`,
      `AGENT_QA_SCENARIOS_DIR={scenarios} AGENT_QA_RECORD_DIR={record} {qa} replay <sid> --session {session}-replay`,
    ],
  },
  {
    tc: "TC18",
    title: "View Category Products",
    steps: [
      "Launch browser.",
      `Navigate to ${automationExerciseUrl}.`,
      "Verify that categories are visible on left side bar.",
      "Click on Women category.",
      "Click on any category link under Women category, for example Dress.",
      "Verify that category page is displayed and confirm text WOMEN - TOPS PRODUCTS.",
      "On left side bar, click on any sub-category link of Men category.",
      "Verify that user is navigated to that category page.",
    ],
    extraConstraints: [
      "The source steps mix Women > Dress with expected text WOMEN - TOPS PRODUCTS. Prefer following the visible subcategory selected and assert the matching category heading; report the source-test mismatch if the expected text does not match the clicked subcategory.",
      "For deterministic coverage, use Women > Tops via #Women a[href=\"/category_products/2\"] to satisfy the documented WOMEN - TOPS PRODUCTS heading, then Men > Tshirts via #Men a[href=\"/category_products/3\"].",
      "Record selectorText headings using DOM text case: Women - Tops Products and Men - Tshirts Products, because CSS uppercases the visual text.",
    ],
  },
  {
    tc: "TC19",
    title: "View & Cart Brand Products",
    steps: [
      "Launch browser.",
      `Navigate to ${automationExerciseUrl}.`,
      "Click on Products button.",
      "Verify that Brands are visible on left side bar.",
      "Click on any brand name.",
      "Verify that user is navigated to brand page and brand products are displayed.",
      "On left side bar, click on any other brand link.",
      "Verify that user is navigated to that brand page and can see products.",
    ],
    extraConstraints: [
      "Use Products page then brand sidebar selectors .brands_products, a[href=\"/brand_products/Polo\"], and a[href=\"/brand_products/H&M\"].",
      "Record selectorText headings with DOM text case: Brand - Polo Products and Brand - H&M Products.",
      "Assert .features_items a[href^=\"/product_details/\"] exists on each brand page.",
    ],
  },
  {
    tc: "TC20",
    title: "Search Products and Verify Cart After Login",
    steps: [
      "Launch browser.",
      `Navigate to ${automationExerciseUrl}.`,
      "Click on Products button.",
      "Verify user is navigated to ALL PRODUCTS page successfully.",
      "Enter product name in search input and click search button.",
      "Verify SEARCHED PRODUCTS is visible.",
      "Verify all the products related to search are visible.",
      "Add those products to cart.",
      "Click Cart button and verify that products are visible in cart.",
      "Click Signup / Login button and submit login details.",
      "Again, go to Cart page.",
      "Verify that those products are visible in cart after login as well.",
    ],
    extraConstraints: disposableAccountConstraints,
  },
  {
    tc: "TC21",
    title: "Add review on product",
    steps: [
      "Launch browser.",
      `Navigate to ${automationExerciseUrl}.`,
      "Click on Products button.",
      "Verify user is navigated to ALL PRODUCTS page successfully.",
      "Click on View Product button.",
      "Verify Write Your Review is visible.",
      "Enter name, email and review.",
      "Click Submit button.",
      "Verify success message Thank you for your review. is visible.",
    ],
    extraConstraints: [
      "Use disposable review data and a unique email address.",
    ],
  },
  {
    tc: "TC22",
    title: "Add to cart from Recommended items",
    steps: [
      "Launch browser.",
      `Navigate to ${automationExerciseUrl}.`,
      "Scroll to bottom of page.",
      "Verify RECOMMENDED ITEMS are visible.",
      "Click on Add To Cart on Recommended product.",
      "Click on View Cart button.",
      "Verify that product is displayed in cart page.",
    ],
  },
  {
    tc: "TC23",
    title: "Verify address details in checkout page",
    steps: [
      "Launch browser.",
      `Navigate to ${automationExerciseUrl}.`,
      "Verify that home page is visible successfully.",
      "Click Signup / Login button.",
      "Fill all details in Signup and create account.",
      "Verify ACCOUNT CREATED! and click Continue button.",
      "Verify Logged in as username at top.",
      "Add products to cart.",
      "Click Cart button.",
      "Verify that cart page is displayed.",
      "Click Proceed To Checkout.",
      "Verify that the delivery address is same address filled at the time registration of account.",
      "Verify that the billing address is same address filled at the time registration of account.",
      "Click Delete Account button.",
      "Verify ACCOUNT DELETED! and click Continue button.",
    ],
    extraConstraints: disposableAccountConstraints,
  },
  {
    tc: "TC24",
    title: "Download Invoice after purchase order",
    steps: [
      "Launch browser.",
      `Navigate to ${automationExerciseUrl}.`,
      "Verify that home page is visible successfully.",
      "Add products to cart.",
      "Click Cart button.",
      "Verify that cart page is displayed.",
      "Click Proceed To Checkout.",
      "Click Register / Login button.",
      "Fill all details in Signup and create account.",
      "Verify ACCOUNT CREATED! and click Continue button.",
      "Verify Logged in as username at top.",
      "Click Cart button.",
      "Click Proceed To Checkout button.",
      "Verify Address Details and Review Your Order.",
      "Enter description in comment text area and click Place Order.",
      "Enter payment details: Name on Card, Card Number, CVC, Expiration date.",
      "Click Pay and Confirm Order button.",
      "Verify success message Your order has been placed successfully!.",
      "Click Download Invoice button and verify invoice is downloaded successfully.",
      "Click Continue button.",
      "Click Delete Account button.",
      "Verify ACCOUNT DELETED! and click Continue button.",
    ],
    extraConstraints: [
      ...disposableAccountConstraints,
      "Use the isolated eval result directory for download verification when supported. Do not rely on the user's default Downloads folder.",
      "If download capture is unsupported, report that framework gap after verifying the order-success page and Download Invoice control are visible.",
    ],
  },
  {
    tc: "TC25",
    title: "Verify Scroll Up using Arrow button and Scroll Down functionality",
    steps: [
      "Launch browser.",
      `Navigate to ${automationExerciseUrl}.`,
      "Verify that home page is visible successfully.",
      "Scroll down page to bottom.",
      "Verify SUBSCRIPTION is visible.",
      "Click on arrow at bottom right side to move upward.",
      "Verify that page is scrolled up and Full-Fledged practice website for Automation Engineers text is visible on screen.",
    ],
  },
  {
    tc: "TC26",
    title: "Verify Scroll Up without Arrow button and Scroll Down functionality",
    steps: [
      "Launch browser.",
      `Navigate to ${automationExerciseUrl}.`,
      "Verify that home page is visible successfully.",
      "Scroll down page to bottom.",
      "Verify SUBSCRIPTION is visible.",
      "Scroll up page to top.",
      "Verify that page is scrolled up and Full-Fledged practice website for Automation Engineers text is visible on screen.",
    ],
  },
];

const automationExerciseCases = automationExerciseTestCases.map(automationExerciseCase);

export const cases: EvalCase[] = [
  {
    id: "saucedemo-checkout",
    suite: "saucedemo",
    name: "Saucedemo checkout reaches final confirmation",
    prompt: `Use the agent-qa skill to record and replay this full checkout flow:

1. Navigate to https://www.saucedemo.com/
2. Log in with username standard_user and password secret_sauce.
3. Add exactly one item to the cart.
4. Open the cart.
5. Start checkout.
6. Fill checkout information with first name Test, last name User, and postal code 12345.
7. Continue checkout.
8. Finish checkout.
9. Verify the final checkout confirmation page is reached.`,
    extraConstraints: [
      "This scenario starts at a public login page. Do not bootstrap profiles.",
      "Fixed values are not unique. Do not use fill-unique for standard_user, secret_sauce, Test, User, or 12345.",
    ],
    scenarioMatches(scenario: unknown): boolean {
      const text = textOf(scenario);
      return text.includes("saucedemo.com") &&
        text.includes("standard_user") &&
        text.includes("secret_sauce") &&
        text.includes("add to cart") &&
        text.includes("checkout") &&
        text.includes("checkout-complete");
    },
  },
  ...qaplaygroundCases,
  ...automationExerciseCases,
];

export function selectCases(caseId?: string, suite?: string, page?: string): EvalCase[] {
  let selected = cases;
  if (caseId) selected = selected.filter((item) => item.id === caseId);
  if (suite) selected = selected.filter((item) => item.suite === suite);
  if (page) selected = selected.filter((item) => item.page === page);
  return selected;
}
