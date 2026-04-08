import json
import subprocess
import os

def run_cmd(cmd):
    try:
        result = subprocess.run(cmd, shell=True, check=True, capture_output=True, text=True)
        return True, result.stdout
    except subprocess.CalledProcessError as e:
        return False, e.stderr

def verify_build():
    print("Verifying build...")
    ok, err = run_cmd("cargo test --workspace --no-run")
    return ok, err

def resolve_conflicts_prefer_ours():
    print("Resolving conflicts: preferring CURRENT (ours/master) over incoming (theirs)...")
    # This command checks out our version for all conflicted files
    run_cmd("git checkout --ours .")
    # Add the resolved files
    run_cmd("git add .")
    # Commit the merge
    # We use -m "Resolve conflicts" but usually git merge already has a message pending if we haven't finished it
    run_cmd("git commit --no-edit")

with open('open_merged_queue.json', 'r') as f:
    open_prs = json.load(f)

with open('closed_unmerged.json', 'r') as f:
    closed_prs = json.load(f)

# Combine and unique by number
all_prs = {pr['number']: pr for pr in open_prs}
for pr in closed_prs:
    all_prs[pr['number']] = pr

prs_list = list(all_prs.values())
prs_list.sort(key=lambda x: x['number'])

print(f"Total PRs to process: {len(prs_list)}")

merged_count = 0
failed_count = 0

for pr in prs_list:
    number = pr['number']
    branch = pr['headRefName']
    oid = pr['headRefOid']
    title = pr['title']
    
    print(f"\n--- Processing PR #{number}: {title} ---")
    
    # 1. Try to fetch/ensure we have the OID
    run_cmd(f"git fetch origin {oid}")
    
    # 2. Attempt merge
    # We use the OID directly to ensure we get exactly what the PR had
    ok, err = run_cmd(f"git merge {oid} -m \"Merge PR #{number}: {title}\"")
    
    if not ok:
        if "CONFLICT" in err:
            print(f"Conflict in PR #{number}. Resolving preferring master...")
            resolve_conflicts_prefer_ours()
        else:
            print(f"Failed to merge PR #{number} for non-conflict reason: {err}")
            run_cmd("git merge --abort")
            failed_count += 1
            continue
    
    # 3. Verify build
    ok, err = verify_build()
    if not ok:
        print(f"Build failed after merging PR #{number}. This shouldn't happen with --ours but may happen if our code itself is broken or merge was logical. Rolling back.")
        run_cmd("git reset --hard HEAD~1")
        failed_count += 1
        continue
    
    print(f"Successfully merged PR #{number}")
    merged_count += 1

print(f"\nFinal Report: Merged {merged_count}, Failed {failed_count}")
