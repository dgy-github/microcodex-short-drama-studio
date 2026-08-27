"""Fixed 17-task advisory story workflow backed by typed Rust capabilities.

Split into modules by responsibility; this package keeps the import surface
that `server.py` and the tests already use:

    graph       the 17-task contract and its execution order
    capability  the typed seam to Rust, plus the failure classes
    context     run-scoped artifacts, budget and first-failure tracking
    prompts     per-task prompt construction
    packaging   artifact normalisation and story-package assembly
    agents      the per-task agent and the contract reviewer
    runner      AdvisoryStoryWorkflow
"""

from __future__ import annotations

from .agents import EPISODE_WRITER_CONCURRENCY, ContractReviewer, StoryAgent
from .capability import (
    CAPABILITY_PROTOCOL,
    Capability,
    PackageValidationFailed,
    ReviewRejected,
    RustCapabilityClient,
)
from .context import WorkflowContext
from .graph import (
    REVIEW_TASKS,
    REVIEW_TYPES,
    TASKS,
    TaskSpec,
    execution_order,
    relevant_context,
    validate_task_graph,
)
from .packaging import (
    canonical_package,
    character_reference,
    episode_index,
    merge_usage,
    normalize_artifact,
    package_schema,
    string_list,
    text_value,
)
from .prompts import (
    SYSTEM,
    build_prompt,
    episode_writer_prompt,
    human_writing_directives,
    review_instruction,
    targeted_revision_context,
)
from .runner import AdvisoryStoryWorkflow

__all__ = [
    "AdvisoryStoryWorkflow",
    "CAPABILITY_PROTOCOL",
    "Capability",
    "ContractReviewer",
    "EPISODE_WRITER_CONCURRENCY",
    "PackageValidationFailed",
    "REVIEW_TASKS",
    "REVIEW_TYPES",
    "ReviewRejected",
    "RustCapabilityClient",
    "SYSTEM",
    "StoryAgent",
    "TASKS",
    "TaskSpec",
    "WorkflowContext",
    "build_prompt",
    "canonical_package",
    "character_reference",
    "episode_index",
    "episode_writer_prompt",
    "execution_order",
    "human_writing_directives",
    "merge_usage",
    "normalize_artifact",
    "package_schema",
    "relevant_context",
    "review_instruction",
    "string_list",
    "targeted_revision_context",
    "text_value",
    "validate_task_graph",
]
