import p from"broken-link-checker";import{setFailed as c}from"@actions/core";import*as h from"@actions/github";var m="# Broken Link Checker";async function k({octokit:n,owner:t,repo:i,prNumber:o}){try{let{data:e}=await n.rest.issues.listComments({owner:t,repo:i,issue_number:o});return e.find(r=>r.body?.includes(m))}catch(e){c("Error finding bot comment: "+e);return}}var g=async n=>{try{let{context:t,getOctokit:i}=h,o=i(process.env.GITHUB_TOKEN),{owner:e,repo:r}=t.repo,s=t.payload.pull_request;s||(console.log("Skipping since this is not a pull request"),process.exit(0));let d=s.head.repo.fork,u=s.number;if(d)return c("The action could not create a Github comment because it is initiated from a forked repo. View the action logs for a list of broken links."),"";let l=await k({octokit:o,owner:e,repo:r,prNumber:u});if(console.log("botComment",l),l){console.log("Updating Comment");let{data:a}=await o.rest.issues.updateComment({owner:e,repo:r,comment_id:l?.id,body:n});return a.html_url}else{console.log("Creating Comment");let{data:a}=await o.rest.issues.createComment({owner:e,repo:r,issue_number:u,body:n});return a.html_url}}catch(t){return c("Error commenting: "+t),""}},f=n=>{let t=`${m}

> **${n.links.length}** broken links found. Links organised below by source page, or page where they were found.
`,i=n.links.reduce((o,e)=>(o[e.base.resolved]||(o[e.base.resolved]=[]),o[e.base.resolved].push(e),o),{});return Object.entries(i).forEach(([o,e],r)=>{t+=`

### ${r+1}) [${new URL(o).pathname}](${o})

| Target Link | Link Text  |
|------|------|
`,e.forEach(s=>{t+=`| [${new URL(s.url.resolved).pathname}](${s.url.resolved}) | "${s.html?.text?.trim().replaceAll(`
`,"")}" |
`})}),n.errors.length&&(t+=`
### Errors
`,n.errors.forEach(o=>{t+=`
${o}
`})),t};async function b(){if(!process.env.GITHUB_TOKEN)throw new Error("GITHUB_TOKEN is required");let n=process.env.VERCEL_PREVIEW_URL||"https://authjs-nextra-docs.vercel.app",t={errors:[],links:[],pages:[],sites:[]},i={excludeExternalLinks:!0,honorRobotExclusions:!1,filterLevel:0,excludedKeywords:[]};new p.SiteChecker(i,{error:e=>{t.errors.push(e)},link:e=>{e.broken&&t.links.push(e)},end:async()=>{if(console.log("end.output.length",t.links.length),t.links.length){let e=t.links.filter(s=>s.broken&&!["HTTP_308"].includes(s.brokenReason));console.log("links404.length",e.length),console.log("links404.output[1]",JSON.stringify(e[1],null,2)),console.log("links404.output[2]",JSON.stringify(e[2],null,2)),console.log("links404.output[3]",JSON.stringify(e[3],null,2));let r=f({errors:t.errors,links:e,pages:[],sites:[]});await g(r),c("Found broken links")}}}).enqueue(n)}b();
