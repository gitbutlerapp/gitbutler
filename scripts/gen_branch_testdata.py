#!/usr/bin/env /usr/bin/python3
import subprocess
import json
import openai
import os

openai.api_key = os.getenv("OPENAI_API_KEY")

try:
    from unidiff import PatchSet
except ImportError as e:
    print(
        "unidiff is not installed, please install it first with: python3 -m pip install unidiff"
    )
    exit(1)


try:
    subprocess.check_output("gh --version", shell=True, text=True)
except subprocess.CalledProcessError as e:
    print("gh is not installed, please install it first from https://cli.github.com/")
    exit(1)


def get_last_n_pr_nums(n_prs):
    list_prs = subprocess.check_output(
        "gh pr list --state merged | head -n %d | awk '{print $1}'" % n_prs,
        shell=True,
        text=True,
    )
    return list_prs.splitlines()


def summarize_hunk(hunk):
    prompt = """
    Summarize the following git diff hunk in less than 80 characters:

    ```
    {hunk}
    ```
    """.format(
        hunk=hunk[0:1000]
    )
    response = openai.ChatCompletion.create(
        model="gpt-3.5-turbo",
        messages=[{"role": "user", "content": prompt}],
        max_tokens=32,
    )
    return response.choices[0].message.content.strip()


def process_pr(pr_number):
    branch_name = subprocess.check_output(
        "gh pr view %s --json headRefName -q '.headRefName'" % pr_number,
        shell=True,
        text=True,
    ).splitlines()[0]
    updated_at = subprocess.check_output(
        "gh pr view %s --json updatedAt -q '.updatedAt'" % pr_number,
        shell=True,
        text=True,
    ).splitlines()[0]
    title = subprocess.check_output(
        "gh pr view %s --json title -q '.title'" % pr_number,
        shell=True,
        text=True,
    ).splitlines()[0]
    body = subprocess.check_output(
        "gh pr view %s --json body -q '.body'" % pr_number,
        shell=True,
        text=True,
    ).splitlines()[0]
    diff = subprocess.check_output("gh pr diff %s" % pr_number, shell=True, text=True)
    patch = PatchSet(diff)
    files = []
    for file in patch:
        hunks = []
        for hunk in file:
            hunk_out = {
                "id": branch_name + ":" + file.path + ":" + str(hunk.target_start),
                "name": summarize_hunk(str(hunk)),
                "diff": str(hunk),
                "kind": "hunk",
                "modifiedAt": updated_at,
                "filePath": file.path,
            }
            hunks.append(hunk_out)
        file_out = {
            "id": branch_name + ":" + file.path,
            "path": file.path,
            "kind": "file",
            "hunks": hunks,
        }
        files.append(file_out)
    branch = {
        "id": branch_name + ":" + pr_number,
        "name": branch_name,
        "active": True,
        "kind": "branch",
        "files": files,
        "description": title + "\n" + body,
    }
    return branch


# prs = get_last_n_pr_nums(4)
prs = [
    "425",
    "429",
    "420",
    "414",
    "409",
    "407",
]  # feel free to paste some some specific PRs

branches = [process_pr(pr) for pr in prs]

with open("scripts/branch_testdata.json", "w") as json_file:
    json.dump(branches, json_file, indent=4)
